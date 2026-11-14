//! Read-only recorded progress, distinct from fresh output-reuse assessment.
use super::{UpscaylRecoveryRequest, check_cancelled, invalid};
use crate::{CancellationToken, PipelineV3StageState, Result, StageRunRecord};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedFrames {
    pub total: u64,
    pub complete: u64,
    pub pending: u64,
    pub active_or_uncertain: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub invalidated: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryUnit {
    pub stage_id: String,
    pub frame_count: u64,
    pub state: PipelineV3StageState,
    pub checkpoint_sha256: Option<String>,
    pub rerun_required: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryInventory {
    pub schema: String,
    pub plan_sha256: String,
    pub manifest_sha256: String,
    pub frames: RecordedFrames,
    pub units: Vec<RecoveryUnit>,
    pub collection_state: PipelineV3StageState,
    pub output_reuse_authority: String,
    pub estimated_remaining_milliseconds: Option<u64>,
}
fn project(
    weights: &BTreeMap<String, u64>,
    records: &[StageRunRecord],
) -> Result<(RecordedFrames, Vec<RecoveryUnit>)> {
    let mut seen = BTreeSet::new();
    let mut counts = RecordedFrames::default();
    let mut units = Vec::new();
    for record in records {
        record.validate()?;
        if !seen.insert(record.stage_id.as_str()) {
            return Err(invalid("recovery manifest contains duplicate stages"));
        }
        let Some(&weight) = weights.get(&record.stage_id) else {
            continue;
        };
        if weight == 0 {
            return Err(invalid("recovery units must contain at least one frame"));
        }
        counts.total = counts
            .total
            .checked_add(weight)
            .ok_or_else(|| invalid("recovery frame count overflow"))?;
        let bucket = match record.state {
            PipelineV3StageState::Complete => &mut counts.complete,
            PipelineV3StageState::Pending => &mut counts.pending,
            PipelineV3StageState::Running | PipelineV3StageState::Validating => {
                &mut counts.active_or_uncertain
            }
            PipelineV3StageState::Failed => &mut counts.failed,
            PipelineV3StageState::Cancelled => &mut counts.cancelled,
            PipelineV3StageState::Invalidated => &mut counts.invalidated,
            PipelineV3StageState::Skipped => {
                return Err(invalid("a required upscale unit cannot be skipped"));
            }
        };
        *bucket = bucket
            .checked_add(weight)
            .ok_or_else(|| invalid("recovery frame count overflow"))?;
        units.push(RecoveryUnit {
            stage_id: record.stage_id.clone(),
            frame_count: weight,
            state: record.state,
            checkpoint_sha256: record
                .checkpoint
                .as_ref()
                .map(|v| v.checkpoint_sha256.clone()),
            rerun_required: record.rerun_required,
        });
    }
    if units.len() != weights.len() {
        return Err(invalid(
            "recovery manifest is missing a planned frame/chunk unit",
        ));
    }
    Ok((counts, units))
}
/// Re-observe request inputs/components and compare the exact native plan. No
/// process is launched, files written, or checkpoint output reuse authorized.
/// A complete count describes retained state; resume owns fresh artifact checks.
pub fn observe(
    request: &UpscaylRecoveryRequest,
    root: &Path,
    token: &CancellationToken,
) -> Result<RecoveryInventory> {
    check_cancelled(token)?;
    let prepared = super::prepare(request, token)?;
    let before = crate::status_v3(root)?;
    if before.payload.plan_sha256 != prepared.plan.plan_sha256 {
        return Err(invalid(
            "recovery request differs from the retained native plan",
        ));
    }
    let expected: BTreeSet<_> = prepared
        .plan
        .payload
        .stages
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    let actual: BTreeSet<_> = before
        .payload
        .stages
        .iter()
        .map(|s| s.stage_id.as_str())
        .collect();
    if actual != expected {
        return Err(invalid(
            "recovery manifest stage set differs from the exact plan",
        ));
    }
    let (frames, units) = project(&prepared.frame_weights, &before.payload.stages)?;
    let collection = if request.configuration.execution.is_per_frame() {
        "collect_upscaled_frames"
    } else {
        "collect_upscaled_chunks"
    };
    let collection_state = before
        .payload
        .stages
        .iter()
        .find(|s| s.stage_id == collection)
        .ok_or_else(|| invalid("recovery collection stage is missing"))?
        .state;
    if crate::status_v3(root)? != before {
        return Err(invalid(
            "recovery state changed during inventory; retry the read",
        ));
    }
    check_cancelled(token)?;
    Ok(RecoveryInventory {schema:"aniflow.upscayl-recovery-inventory/v1".into(),plan_sha256:prepared.plan.plan_sha256,
        manifest_sha256:before.manifest_sha256,frames,units,collection_state,
        output_reuse_authority:"none; explicit native resume must freshly validate outputs and resolve interrupted attempts".into(),
        estimated_remaining_milliseconds:None})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stage(id: &str, state: PipelineV3StageState) -> StageRunRecord {
        StageRunRecord {
            stage_id: id.into(),
            state,
            checkpoint: None,
            compatibility: None,
            message: None,
            rerun_required: false,
            excluded_checkpoints: Vec::new(),
        }
    }
    #[test]
    fn chunk_weights_and_uncertain_work_are_not_completion() {
        let weights = BTreeMap::from([("one".into(), 8), ("two".into(), 3)]);
        let (counts, _) = project(
            &weights,
            &[
                stage("one", PipelineV3StageState::Validating),
                stage("two", PipelineV3StageState::Pending),
            ],
        )
        .unwrap();
        assert_eq!(counts.total, 11);
        assert_eq!(counts.active_or_uncertain, 8);
        assert_eq!(counts.pending, 3);
        assert_eq!(counts.complete, 0);
    }
    #[test]
    fn failure_and_cancellation_remain_separate() {
        let weights = BTreeMap::from([("one".into(), 1), ("two".into(), 1)]);
        let (counts, _) = project(
            &weights,
            &[
                stage("one", PipelineV3StageState::Failed),
                stage("two", PipelineV3StageState::Cancelled),
            ],
        )
        .unwrap();
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.cancelled, 1);
        assert_eq!(counts.complete, 0);
    }
    #[test]
    fn missing_duplicate_or_skipped_unit_refuses() {
        let weights = BTreeMap::from([("one".into(), 1)]);
        let pending = stage("one", PipelineV3StageState::Pending);
        assert!(project(&weights, &[]).is_err());
        assert!(project(&weights, &[pending.clone(), pending]).is_err());
        assert!(project(&weights, &[stage("one", PipelineV3StageState::Skipped)]).is_err());
    }
    #[test]
    fn complete_without_native_checkpoint_is_never_counted() {
        assert!(
            project(
                &BTreeMap::from([("one".into(), 1)]),
                &[stage("one", PipelineV3StageState::Complete)]
            )
            .is_err()
        );
    }
    #[test]
    fn frame_count_overflow_and_zero_units_refuse() {
        let states = [
            stage("one", PipelineV3StageState::Pending),
            stage("two", PipelineV3StageState::Pending),
        ];
        assert!(
            project(
                &BTreeMap::from([("one".into(), u64::MAX), ("two".into(), 1)]),
                &states
            )
            .is_err()
        );
        assert!(project(&BTreeMap::from([("one".into(), 0)]), &states[..1]).is_err());
    }
}
