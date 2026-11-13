//! Conditional serial-work estimates from exact retained execution evidence.
//! No output reuse, process launch or completion deadline is authorized here.
use super::{
    UpscaylRecoveryRequest, check_cancelled, invalid,
    inventory::{self, RecoveryInventory},
};
use crate::frame_io::{ensure_canonical_directory, identity};
use crate::{
    CancellationToken, ProviderExecutionOutcome, ProviderExecutionReport, Result, StageCheckpoint,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};

const MAX_DOCUMENT: u64 = 8_388_608;
const MAX_TOTAL: u64 = 67_108_864;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimingSample {
    pub stage_id: String,
    pub checkpoint_sha256: String,
    pub execution_report_sha256: String,
    pub frame_count: u64,
    pub provider_duration_milliseconds: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryTiming {
    pub samples: Vec<TimingSample>,
    pub measured_frames: u64,
    pub measured_provider_milliseconds: u64,
    pub remaining_frames: u64,
    pub estimated_remaining_milliseconds: Option<u64>,
    pub basis: String,
    pub exclusions: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryProgress {
    pub schema: String,
    pub inventory: RecoveryInventory,
    pub timing: RecoveryTiming,
    pub native_qualified: bool,
    pub creative_approval: String,
}
fn estimate(samples: &[TimingSample], remaining: u64) -> Result<(u64, u64, Option<u64>)> {
    let mut frames = 0u64;
    let mut duration = 0u64;
    for sample in samples {
        if sample.frame_count == 0 {
            return Err(invalid("zero-frame timing sample"));
        }
        frames = frames
            .checked_add(sample.frame_count)
            .ok_or_else(|| invalid("timing frame sum overflow"))?;
        duration = duration
            .checked_add(sample.provider_duration_milliseconds)
            .ok_or_else(|| invalid("timing duration sum overflow"))?;
    }
    let estimate = if remaining == 0 {
        Some(0)
    } else if frames == 0 || duration == 0 {
        None
    } else {
        Some(
            u64::try_from(
                (u128::from(duration) * u128::from(remaining)).div_ceil(u128::from(frames)),
            )
            .map_err(|_| invalid("remaining-work estimate exceeds its bound"))?,
        )
    };
    Ok((frames, duration, estimate))
}
struct Evidence {
    total: u64,
    pins: Vec<(PathBuf, (String, u64))>,
}
impl Evidence {
    fn read<T: DeserializeOwned>(&mut self, root: &Path, relative: &str) -> Result<T> {
        let relative_path = Path::new(relative);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            return Err(invalid(
                "timing evidence locator must be a confined relative path",
            ));
        }
        let path = root.join(relative_path);
        let pin = identity(&path, MAX_DOCUMENT)?;
        self.total = self
            .total
            .checked_add(pin.1)
            .filter(|n| *n <= MAX_TOTAL)
            .ok_or_else(|| invalid("timing evidence exceeds 64 MiB"))?;
        let value = crate::destination::local::read_bounded(&path, MAX_DOCUMENT)?;
        if identity(&path, MAX_DOCUMENT)? != pin {
            return Err(invalid("timing evidence changed while reading"));
        }
        self.pins.push((path, pin));
        Ok(value)
    }
}
/// Replan the exact existing native request, then read only the completed
/// frame/chunk units' checkpoint-bound successful provider durations. A recorded
/// complete unit is not a fresh verification of its output PNGs.
pub fn observe(
    request: &UpscaylRecoveryRequest,
    root: &Path,
    token: &CancellationToken,
) -> Result<RecoveryProgress> {
    check_cancelled(token)?;
    ensure_canonical_directory(root)?;
    let inventory = inventory::observe(request, root, token)?;
    let prepared = super::prepare(request, token)?;
    let before = crate::status_v3(root)?;
    if before.manifest_sha256 != inventory.manifest_sha256
        || before.payload.plan_sha256 != prepared.plan.plan_sha256
    {
        return Err(invalid("recovery state changed before timing observation"));
    }
    let mut evidence = Evidence {
        total: 0,
        pins: Vec::new(),
    };
    let mut samples = Vec::new();
    for unit in &inventory.units {
        check_cancelled(token)?;
        if unit.state != crate::PipelineV3StageState::Complete {
            continue;
        }
        let record = before
            .payload
            .stages
            .iter()
            .find(|s| s.stage_id == unit.stage_id)
            .ok_or_else(|| invalid("timing stage is absent"))?;
        let reference = record
            .checkpoint
            .as_ref()
            .ok_or_else(|| invalid("completed timing stage lacks a checkpoint"))?;
        reference.validate()?;
        let checkpoint: StageCheckpoint = evidence.read(root, &reference.relative_path)?;
        checkpoint.validate()?;
        let stage = prepared
            .plan
            .payload
            .stages
            .iter()
            .find(|s| s.id == unit.stage_id)
            .ok_or_else(|| invalid("timing unit is outside exact plan"))?;
        if checkpoint.checkpoint_sha256 != reference.checkpoint_sha256
            || reference.stage_id != unit.stage_id
            || checkpoint.payload.stage_id != unit.stage_id
            || checkpoint.payload.plan_sha256 != prepared.plan.plan_sha256
            || checkpoint.payload.provider_lock_sha256 != stage.provider_lock.lock_sha256
        {
            return Err(invalid(
                "timing checkpoint differs from current exact plan/stage/provider",
            ));
        }
        checkpoint.payload.execution_report.validate()?;
        let binding = crate::provider::canonical_sha256(
            &serde_json::json!({"stage_id":unit.stage_id,"stage_invocation_sha256":checkpoint.payload.stage_invocation_sha256,"report_sha256":checkpoint.payload.execution_report.sha256}),
        )?;
        if checkpoint.payload.execution_report.relative_path
            != format!("providers/{}-{binding}.report.json", unit.stage_id)
        {
            return Err(invalid(
                "timing report locator differs from its exact stage invocation",
            ));
        }
        let report: ProviderExecutionReport =
            evidence.read(root, &checkpoint.payload.execution_report.relative_path)?;
        report.validate()?;
        if report.report_sha256 != checkpoint.payload.execution_report.sha256
            || report.payload.provider_lock != stage.provider_lock
            || report.payload.outcome != ProviderExecutionOutcome::Succeeded
        {
            return Err(invalid(
                "timing report is not the successful execution bound by this checkpoint",
            ));
        }
        samples.push(TimingSample {
            stage_id: unit.stage_id.clone(),
            checkpoint_sha256: checkpoint.checkpoint_sha256,
            execution_report_sha256: report.report_sha256,
            frame_count: unit.frame_count,
            provider_duration_milliseconds: report.payload.duration_milliseconds,
        });
    }
    let remaining = inventory
        .frames
        .total
        .checked_sub(inventory.frames.complete)
        .ok_or_else(|| invalid("recorded recovery counts differ"))?;
    let (measured_frames, duration, estimated) = estimate(&samples, remaining)?;
    if measured_frames != inventory.frames.complete {
        return Err(invalid(
            "timing sample coverage differs from completed units",
        ));
    }
    for (path, pin) in &evidence.pins {
        check_cancelled(token)?;
        if identity(path, MAX_DOCUMENT)? != *pin {
            return Err(invalid(
                "retained timing evidence changed during observation",
            ));
        }
    }
    if crate::status_v3(root)? != before || inventory::observe(request, root, token)? != inventory {
        return Err(invalid(
            "recovery inputs/state changed during timing observation",
        ));
    }
    check_cancelled(token)?;
    Ok(RecoveryProgress {schema:"aniflow.upscayl-recovery-progress/v1".into(),inventory,timing:RecoveryTiming {samples,measured_frames,measured_provider_milliseconds:duration,
        remaining_frames:remaining,estimated_remaining_milliseconds:estimated,
        basis:"conditional serial work at the frame-weighted mean of completed provider durations; no deadline".into(),
        exclusions:"collector, validation, retries, contention, active-unit elapsed time, future host changes and full-run completion are not estimated; zero/no measured duration yields unavailable".into()},
        native_qualified:false,creative_approval:"none".into()})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(frames: u64, ms: u64) -> TimingSample {
        TimingSample {
            stage_id: "synthetic".into(),
            checkpoint_sha256: "a".repeat(64),
            execution_report_sha256: "b".repeat(64),
            frame_count: frames,
            provider_duration_milliseconds: ms,
        }
    }
    #[test]
    fn frame_weighted_chunk_estimate_uses_measured_work_and_rounds_up() {
        assert_eq!(
            estimate(&[sample(2, 700), sample(1, 301)], 7).unwrap(),
            (3, 1001, Some(2336))
        );
    }
    #[test]
    fn missing_or_submillisecond_samples_do_not_invent_throughput() {
        assert_eq!(estimate(&[], 8).unwrap().2, None);
        assert_eq!(estimate(&[sample(2, 0)], 8).unwrap().2, None);
        assert_eq!(estimate(&[], 0).unwrap().2, Some(0));
    }
    #[test]
    fn invalid_or_overflowing_measurements_refuse() {
        assert!(estimate(&[sample(0, 1)], 8).is_err());
        assert!(estimate(&[sample(1, u64::MAX), sample(1, 1)], 8).is_err());
        assert!(estimate(&[sample(1, u64::MAX)], u64::MAX).is_err());
    }
    #[test]
    fn absolute_or_traversing_timing_locators_refuse_before_workspace_access() {
        let mut evidence = Evidence {
            total: 0,
            pins: Vec::new(),
        };
        for path in [
            "/outside/report.json",
            "../report.json",
            "stages/../report.json",
        ] {
            assert!(
                evidence
                    .read::<serde_json::Value>(Path::new("/synthetic/not-opened"), path)
                    .is_err()
            );
        }
        assert_eq!(evidence.total, 0);
        assert!(evidence.pins.is_empty());
    }
}
