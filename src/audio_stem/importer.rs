//! Read-only import of retained, completed separation authority.
//!
//! Provider-authored evidence remains opaque. The import derives its relationship
//! from the typed Pipeline v3 input/output binding and independently checks PCM
//! clocks and bytes. It never executes a separation provider or needs its cache.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use serde_json::json;
use sha2::{Digest as _, Sha256};

use super::{AudioStemLineage, AudioStemTimingBasis, StemSelection};
use crate::audio_analysis::{
    AudioArtifactReference, AudioFrameRange, AudioRationalTime, AudioScope, AudioSource,
    AudioStemIdentity,
};
use crate::audio_inspection::wav;
use crate::provider::{canonical_sha256, decode_json};
use crate::{
    ArtifactEvidence, ArtifactKind, ArtifactObservation, ArtifactRole, CancellationToken, Error,
    ErrorCategory, PipelineInputBinding, PipelineInputKind, PipelineV3Plan, PipelineV3RunManifest,
    PipelineV3RunState, PipelineV3StageState, PipelineV3Workspace, ProviderExecutionOutcome,
    ProviderExecutionReport, ProviderLock, ResolvedPipelineStage, Result, StageCheckpoint,
    StreamRole,
};

const MAXIMUM_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
const MAXIMUM_AUDIO_BYTES: u64 = 256 * 1024 * 1024;
const MAXIMUM_AUTHORITY_DIRECTORY_FILES: usize = 128;
const MAXIMUM_STEMS: usize = 8;
const INTEGRITY_CONTRACT: &str = "aniflow.validation/artifact-integrity/v1";

pub(crate) struct VerifiedStemImport {
    pub selected_path: PathBuf,
    pub evidence_path: PathBuf,
    pub lineage: AudioStemLineage,
    pub authority_bindings: Vec<PipelineInputBinding>,
}

#[derive(Default)]
struct Authority {
    artifacts: Vec<AudioArtifactReference>,
    bindings: Vec<PipelineInputBinding>,
}

impl Authority {
    fn add(&mut self, id: &str, path: &Path, bytes: &[u8]) {
        self.artifacts.push(AudioArtifactReference {
            id: id.to_owned(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            byte_size: bytes.len() as u64,
        });
        self.bindings.push(PipelineInputBinding::new(id, path));
    }
}

fn refused(message: impl Into<String>) -> Error {
    Error::new(
        ErrorCategory::Input,
        format!("stem lineage refused: {}", message.into()),
    )
}

fn cancelled(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(Error::new(
            ErrorCategory::Execution,
            "stem lineage import cancelled",
        ));
    }
    Ok(())
}

fn canonical_existing(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| refused("current directory is unavailable"))?
            .join(path)
    };
    if absolute
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(refused("paths must not contain parent traversal"));
    }
    let canonical =
        fs::canonicalize(&absolute).map_err(|_| refused("input path is unavailable"))?;
    if canonical != absolute {
        return Err(refused(
            "input paths must have canonical, nonsymlink components",
        ));
    }
    Ok(canonical)
}

fn confined_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || relative.contains('\\')
        || relative
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || relative.chars().any(char::is_control)
    {
        return Err(refused(
            "retained authority has a nonportable workspace path",
        ));
    }
    let mut current = root.to_path_buf();
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(part) = component else {
            return Err(refused("retained authority escaped its workspace"));
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| refused("retained artifact or authority is missing"))?;
        if metadata.file_type().is_symlink()
            || (components.peek().is_some() && !metadata.is_dir())
            || (components.peek().is_none() && !metadata.is_file())
        {
            return Err(refused(
                "retained artifacts require real directories and regular files",
            ));
        }
    }
    Ok(current)
}

fn read_document(path: &Path, cancellation: &CancellationToken) -> Result<Vec<u8>> {
    cancelled(cancellation)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| refused("authority is unavailable"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAXIMUM_DOCUMENT_BYTES {
        return Err(refused(
            "authority must be a nonempty regular JSON file no larger than 8 MiB",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| refused("authority cannot be opened"))?
        .take(MAXIMUM_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| refused("authority cannot be read"))?;
    cancelled(cancellation)?;
    if bytes.len() as u64 != metadata.len() || bytes.len() as u64 > MAXIMUM_DOCUMENT_BYTES {
        return Err(refused("authority changed or exceeded its document bound"));
    }
    Ok(bytes)
}

fn guard_workspace(
    workspace: &PipelineV3Workspace,
    cancellation: &CancellationToken,
) -> Result<()> {
    // Bound the existing generic status reader before it walks historical files.
    for directory in [
        workspace.manifests(),
        workspace.checkpoints(),
        workspace.providers(),
    ] {
        let entries =
            fs::read_dir(directory).map_err(|_| refused("authority directory is unavailable"))?;
        for (index, entry) in entries.enumerate() {
            cancelled(cancellation)?;
            if index >= MAXIMUM_AUTHORITY_DIRECTORY_FILES {
                return Err(refused(
                    "separation authority exceeds 128 files per directory",
                ));
            }
            let entry = entry.map_err(|_| refused("authority entry is unavailable"))?;
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| refused("authority metadata is unavailable"))?;
            if !metadata.is_file() || metadata.len() > MAXIMUM_DOCUMENT_BYTES {
                return Err(refused(
                    "separation authority contains an unsupported entry",
                ));
            }
        }
    }
    Ok(())
}

fn observe_file(
    path: &Path,
    id: &str,
    maximum_bytes: u64,
    cancellation: &CancellationToken,
) -> Result<AudioArtifactReference> {
    cancelled(cancellation)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| refused("artifact is unavailable"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(refused(
            "artifact is empty, oversized, or not a regular file",
        ));
    }
    let mut file = fs::File::open(path).map_err(|_| refused("artifact cannot be opened"))?;
    let mut digest = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        cancelled(cancellation)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_| refused("artifact cannot be read"))?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > maximum_bytes {
            return Err(refused("artifact grew beyond its byte bound"));
        }
        digest.update(&buffer[..read]);
    }
    if count != metadata.len() {
        return Err(refused("artifact changed during import"));
    }
    Ok(AudioArtifactReference {
        id: id.to_owned(),
        sha256: format!("{:x}", digest.finalize()),
        byte_size: count,
    })
}

fn audio_source(artifact: AudioArtifactReference, path: &Path) -> Result<AudioSource> {
    let wave = wav::inspect(path)?;
    Ok(AudioSource {
        artifact,
        stream_index: 0,
        sample_rate_hz: wave.sample_rate_hz,
        channels: wave.channels,
        frame_count: wave.frame_count,
        origin: AudioRationalTime {
            numerator: 0,
            denominator: 1,
        },
        stem: None,
    })
}

fn verify_duration(mix: &AudioSource, stem: &AudioSource, tolerance: u16) -> Result<()> {
    let mix_scaled = u128::from(mix.frame_count) * u128::from(stem.sample_rate_hz);
    let stem_scaled = u128::from(stem.frame_count) * u128::from(mix.sample_rate_hz);
    let difference = mix_scaled.abs_diff(stem_scaled) * 1000;
    let limit =
        u128::from(tolerance) * u128::from(mix.sample_rate_hz) * u128::from(stem.sample_rate_hz);
    if difference > limit {
        return Err(refused(
            "stem duration exceeds the exact requested mix-duration tolerance",
        ));
    }
    Ok(())
}

fn typed_audio(artifact: &ArtifactEvidence) -> bool {
    artifact.kind == ArtifactKind::File
        && artifact.file_count == 1
        && artifact.artifact_type == "audio/wav"
        && artifact.artifact_role == ArtifactRole::TemporalComponent
        && artifact.stream_role == Some(StreamRole::Audio)
}

fn verify_input(
    plan: &PipelineV3Plan,
    stage: &ResolvedPipelineStage,
    checkpoint: &StageCheckpoint,
) -> Result<()> {
    if !stage.depends_on.is_empty()
        || !checkpoint.payload.dependencies.is_empty()
        || stage.inputs.len() != 1
        || stage.inputs[0].artifacts.len() != 1
        || checkpoint.payload.inputs.len() != 1
    {
        return Err(refused(
            "only a separation stage with one direct original-mix input is supported",
        ));
    }
    let input = &checkpoint.payload.inputs[0];
    let planned = plan
        .payload
        .inputs
        .iter()
        .find(|candidate| candidate.id == input.id)
        .ok_or_else(|| refused("original mix is not an explicit pipeline input"))?;
    if !typed_audio(input)
        || input.relative_path.is_some()
        || stage.inputs[0].port != input.port
        || stage.inputs[0].artifacts[0] != input.id
        || planned.kind != PipelineInputKind::File
        || planned.file_count != input.file_count
        || planned.byte_count != input.byte_count
        || planned.content_sha256 != input.sha256
        || planned.artifact_type != input.artifact_type
        || planned.artifact_role != input.artifact_role
        || planned.stream_role != input.stream_role
    {
        return Err(refused(
            "original mix identity differs between plan and accepted checkpoint",
        ));
    }
    Ok(())
}

fn verify_report(
    root: &Path,
    stage: &ResolvedPipelineStage,
    checkpoint: &StageCheckpoint,
    authority: &mut Authority,
    cancellation: &CancellationToken,
) -> Result<()> {
    let reference = &checkpoint.payload.execution_report;
    let path = confined_path(root, &reference.relative_path)?;
    let bytes = read_document(&path, cancellation)?;
    let report = ProviderExecutionReport::from_json_slice(&bytes)?;
    if report.report_sha256 != reference.sha256
        || report.payload.provider_lock != stage.provider_lock
        || report.payload.outcome != ProviderExecutionOutcome::Succeeded
    {
        return Err(refused(
            "execution report does not prove the accepted provider invocation",
        ));
    }
    // Match the existing Pipeline v3 semantic identities exactly. Dependencies
    // are empty in this bounded import profile, not omitted or reconstructed.
    let invocation = canonical_sha256(&json!({
        "provider_invocation_schema": crate::PROVIDER_INVOCATION_SCHEMA_V1,
        "execution_semantics": crate::PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1,
        "execution_bounds": report.payload.bounds,
        "stage_plan_sha256": canonical_sha256(stage)?,
        "provider_lock_sha256": stage.provider_lock.lock_sha256,
        "inputs": checkpoint.payload.inputs,
        "dependencies": [],
    }))?;
    let binding = canonical_sha256(&json!({
        "stage_id": stage.id,
        "stage_invocation_sha256": invocation,
        "report_sha256": report.report_sha256,
    }))?;
    if invocation != checkpoint.payload.stage_invocation_sha256
        || reference.relative_path != format!("providers/{}-{binding}.report.json", stage.id)
    {
        return Err(refused(
            "execution report is not bound to this exact stage and invocation",
        ));
    }
    let mut expected: Vec<_> = checkpoint
        .payload
        .outputs
        .iter()
        .map(|output| ArtifactObservation {
            port: output.port.clone(),
            relative_path: output.relative_path.clone().unwrap_or_default(),
            kind: output.kind,
            file_count: output.file_count,
            byte_count: output.byte_count,
            sha256: output.sha256.clone(),
        })
        .collect();
    expected.sort_by(|left, right| left.port.cmp(&right.port));
    if report.payload.outputs != expected {
        return Err(refused(
            "execution report output identities differ from the checkpoint",
        ));
    }
    authority.add("stem_authority_report", &path, &bytes);
    Ok(())
}

fn verify_validations(
    root: &Path,
    stage: &ResolvedPipelineStage,
    checkpoint: &StageCheckpoint,
    authority: &mut Authority,
    cancellation: &CancellationToken,
) -> Result<()> {
    if stage.validations.len() != checkpoint.payload.outputs.len()
        || stage.validations.len() != checkpoint.payload.validations.len()
    {
        return Err(refused(
            "each declared output requires exactly one accepted integrity validation",
        ));
    }
    let mut validated = BTreeSet::new();
    for (index, (expected, recorded)) in stage
        .validations
        .iter()
        .zip(&checkpoint.payload.validations)
        .enumerate()
    {
        let output = checkpoint
            .payload
            .outputs
            .iter()
            .find(|output| output.id == expected.artifact)
            .ok_or_else(|| refused("validation references an undeclared output"))?;
        if !validated.insert(output.id.as_str())
            || expected.contract != INTEGRITY_CONTRACT
            || recorded.id != expected.id
            || recorded.contract != expected.contract
            || recorded.artifact_id != expected.artifact
            || recorded.artifact_sha256 != output.sha256
            || !recorded.accepted
        {
            return Err(refused(
                "declared and accepted artifact-integrity validations differ",
            ));
        }
        let value = json!({
            "schema": "aniflow.artifact-integrity-validation/v1",
            "validation_id": recorded.id,
            "contract": recorded.contract,
            "artifact_id": recorded.artifact_id,
            "artifact_sha256": recorded.artifact_sha256,
            "accepted": true,
        });
        let digest = canonical_sha256(&value)?;
        let relative = format!(
            "providers/{}-{}-{digest}.validation.json",
            stage.id, recorded.id
        );
        if recorded.evidence.sha256 != digest || recorded.evidence.relative_path != relative {
            return Err(refused(
                "integrity validation reference differs from its exact content identity",
            ));
        }
        let path = confined_path(root, &relative)?;
        let bytes = read_document(&path, cancellation)?;
        let observed: serde_json::Value = decode_json(&bytes, "stem integrity evidence")?;
        if observed != value {
            return Err(refused("integrity validation artifact changed"));
        }
        authority.add(&format!("stem_authority_validation_{index}"), &path, &bytes);
    }
    Ok(())
}

fn completed_authority(
    root: &Path,
    selection: &StemSelection,
    authority: &mut Authority,
    cancellation: &CancellationToken,
) -> Result<(PipelineV3Plan, PipelineV3RunManifest, StageCheckpoint)> {
    let workspace = PipelineV3Workspace::open_read_only(root)?;
    guard_workspace(&workspace, cancellation)?;
    let plan_path = confined_path(root, "plan/plan.json")?;
    let plan_bytes = read_document(&plan_path, cancellation)?;
    let plan = PipelineV3Plan::from_json_slice(&plan_bytes)
        .map_err(|error| refused(format!("invalid retained plan: {error}")))?;
    let manifest = crate::status_v3(root)?;
    cancelled(cancellation)?;
    if manifest.payload.state != PipelineV3RunState::Complete
        || manifest.payload.plan_sha256 != plan.plan_sha256
    {
        return Err(refused("separation run must be complete"));
    }
    let stage = plan
        .payload
        .stages
        .iter()
        .find(|stage| stage.id == selection.stage_id)
        .ok_or_else(|| refused("selected separation stage is undeclared"))?;
    if stage.capability.id != "aniflow/audio.separate" || stage.capability.version != "1.0.0" {
        return Err(refused(
            "selected stage does not declare the supported separation capability",
        ));
    }
    let recorded = manifest
        .payload
        .stages
        .iter()
        .find(|recorded| recorded.stage_id == stage.id)
        .ok_or_else(|| refused("selected separation stage is missing from current run state"))?;
    if recorded.state != PipelineV3StageState::Complete {
        return Err(refused("selected separation stage is not complete"));
    }
    let reference = recorded
        .checkpoint
        .as_ref()
        .ok_or_else(|| refused("completed stage lacks its checkpoint"))?;
    let checkpoint_path = confined_path(root, &reference.relative_path)?;
    let checkpoint_bytes = read_document(&checkpoint_path, cancellation)?;
    let checkpoint = StageCheckpoint::from_json_slice(&checkpoint_bytes)?;
    if checkpoint.reference()? != *reference
        || checkpoint.payload.plan_sha256 != plan.plan_sha256
        || checkpoint.payload.provider_lock_sha256 != stage.provider_lock.lock_sha256
    {
        return Err(refused(
            "selected checkpoint does not match current manifest, plan, and provider lock",
        ));
    }
    verify_input(&plan, stage, &checkpoint)?;
    let manifest_relative = format!("state/manifests/{:020}.json", manifest.payload.revision);
    let manifest_path = confined_path(root, &manifest_relative)?;
    let manifest_bytes = read_document(&manifest_path, cancellation)?;
    if PipelineV3RunManifest::from_json_slice(&manifest_bytes)? != manifest {
        return Err(refused("current separation manifest changed during import"));
    }
    let lock_path = confined_path(
        root,
        &format!(
            "providers/{}-{}.lock.json",
            stage.id, stage.provider_lock.lock_sha256
        ),
    )?;
    let lock_bytes = read_document(&lock_path, cancellation)?;
    if ProviderLock::from_json_slice(&lock_bytes)? != stage.provider_lock {
        return Err(refused(
            "retained provider lock differs from the immutable plan",
        ));
    }
    authority.add("stem_authority_plan", &plan_path, &plan_bytes);
    authority.add("stem_authority_manifest", &manifest_path, &manifest_bytes);
    authority.add(
        "stem_authority_checkpoint",
        &checkpoint_path,
        &checkpoint_bytes,
    );
    authority.add("stem_authority_lock", &lock_path, &lock_bytes);
    verify_report(root, stage, &checkpoint, authority, cancellation)?;
    verify_validations(root, stage, &checkpoint, authority, cancellation)?;
    Ok((plan, manifest, checkpoint))
}

pub(crate) fn import_selection(
    original_mix: &Path,
    selection: &StemSelection,
    cancellation: &CancellationToken,
) -> Result<VerifiedStemImport> {
    selection.validate()?;
    cancelled(cancellation)?;
    let root = canonical_existing(&selection.run_directory)?;
    let mix_path = canonical_existing(original_mix)?;
    let mut authority = Authority::default();
    let (plan, manifest, checkpoint) =
        completed_authority(&root, selection, &mut authority, cancellation)?;
    let stage = plan
        .payload
        .stages
        .iter()
        .find(|stage| stage.id == selection.stage_id)
        .ok_or_else(|| refused("selected stage is unavailable"))?;
    let mix_artifact = observe_file(&mix_path, "original_mix", MAXIMUM_AUDIO_BYTES, cancellation)?;
    let original = &checkpoint.payload.inputs[0];
    if mix_artifact.sha256 != original.sha256 || mix_artifact.byte_size != original.byte_count {
        return Err(refused(
            "original mix bytes differ from the separation input",
        ));
    }
    let mix = audio_source(mix_artifact, &mix_path)?;
    let planned_outputs: Vec<_> = stage
        .outputs
        .iter()
        .flat_map(|binding| {
            binding
                .artifacts
                .iter()
                .map(move |artifact| (binding.port.as_str(), artifact))
        })
        .collect();
    if planned_outputs.len() != checkpoint.payload.outputs.len()
        || !(2..=MAXIMUM_STEMS + 1).contains(&planned_outputs.len())
    {
        return Err(refused(
            "separation requires one to eight declared stems and one evidence artifact",
        ));
    }
    let mut evidence = None;
    let mut selected = None;
    let mut stems = 0;
    for ((port, planned), recorded) in planned_outputs.into_iter().zip(&checkpoint.payload.outputs)
    {
        cancelled(cancellation)?;
        if recorded.id != planned.id
            || recorded.port != port
            || recorded.relative_path.as_deref() != Some(planned.relative_path.as_str())
            || planned.kind != Some(PipelineInputKind::File)
            || recorded.kind != ArtifactKind::File
            || recorded.file_count != 1
            || recorded.artifact_type != planned.artifact_type
            || recorded.artifact_role != planned.artifact_role
            || recorded.stream_role != planned.stream_role
        {
            return Err(refused(
                "checkpoint output does not match its exact declared path and role",
            ));
        }
        // Public output aliases use port "final" and may have a different ID
        // from the stage artifact (Demucs separation_evidence -> evidence).
        let public: Vec<_> = plan
            .payload
            .outputs
            .iter()
            .filter(|output| output.artifact == recorded.id)
            .collect();
        if public.len() != 1 {
            return Err(refused(
                "separation artifacts require one unambiguous declared public output",
            ));
        }
        let mut expected_public = recorded.clone();
        expected_public.id = public[0].id.clone();
        expected_public.port = "final".to_owned();
        let integrity = stage
            .validations
            .iter()
            .find(|validation| validation.artifact == recorded.id)
            .ok_or_else(|| refused("public separation artifact lacks its integrity validation"))?;
        if !public[0].required_validations.contains(&integrity.id)
            || !manifest
                .payload
                .outputs
                .iter()
                .any(|output| output == &expected_public)
        {
            return Err(refused(
                "selected separation outputs must remain accepted public run artifacts",
            ));
        }
        let path = confined_path(&root, &planned.relative_path)?;
        let audio = typed_audio(recorded);
        let observation = observe_file(
            &path,
            &recorded.id,
            if audio {
                MAXIMUM_AUDIO_BYTES
            } else {
                MAXIMUM_DOCUMENT_BYTES
            },
            cancellation,
        )?;
        if observation.sha256 != recorded.sha256 || observation.byte_size != recorded.byte_count {
            return Err(refused(
                "separation artifact bytes changed after checkpoint acceptance",
            ));
        }
        if audio {
            stems += 1;
            let mut source = audio_source(observation.clone(), &path)?;
            verify_duration(&mix, &source, selection.duration_tolerance_milliseconds)?;
            if observe_file(&path, &recorded.id, MAXIMUM_AUDIO_BYTES, cancellation)? != observation
            {
                return Err(refused("stem changed during independent PCM inspection"));
            }
            if recorded.id == selection.stem_id {
                source.artifact.id = "source_audio".to_owned();
                selected = Some((path, source, recorded.port.clone()));
            }
        } else if recorded.artifact_role == ArtifactRole::ValidationEvidence
            && recorded.stream_role.is_none()
            && evidence.is_none()
        {
            let mut reference = observation;
            reference.id = "separation_evidence".to_owned();
            evidence = Some((path, reference));
        } else {
            return Err(refused(
                "separation contains an unsupported or ambiguous output role",
            ));
        }
    }
    if stems == 0 || stems > MAXIMUM_STEMS {
        return Err(refused("separation stem count is unsupported"));
    }
    let (selected_path, mut selected_stem, source_stem_port) = selected
        .ok_or_else(|| refused("selected stem identity was not declared as an audio output"))?;
    let (evidence_path, relationship_evidence) = evidence
        .ok_or_else(|| refused("separation lacks its declared relationship evidence artifact"))?;
    let channels: Vec<u16> = (0..selected_stem.channels).collect();
    let range = AudioFrameRange {
        start: 0,
        end: selected_stem.frame_count,
    };
    if selection
        .channels
        .as_ref()
        .is_some_and(|requested| requested != &channels)
        || selection
            .range
            .as_ref()
            .is_some_and(|requested| requested != &range)
    {
        return Err(refused(
            "only all channels and the complete stem range are supported",
        ));
    }
    selected_stem.stem = Some(AudioStemIdentity {
        id: selection.stem_id.clone(),
        original_mix: mix.artifact.clone(),
        relationship_evidence_id: "stem_lineage".to_owned(),
    });
    if observe_file(&mix_path, "original_mix", MAXIMUM_AUDIO_BYTES, cancellation)? != mix.artifact {
        return Err(refused("original mix changed during lineage import"));
    }
    let lineage = AudioStemLineage {
        original_mix: mix,
        selected_stem,
        stem_id: selection.stem_id.clone(),
        source_stage_id: stage.id.clone(),
        source_mix_artifact_id: original.id.clone(),
        source_stem_artifact_id: selection.stem_id.clone(),
        source_stem_port,
        scope: AudioScope {
            channels,
            stem_id: Some(selection.stem_id.clone()),
        },
        range,
        relationship_evidence,
        authority_artifacts: authority.artifacts,
        source_plan_sha256: plan.plan_sha256.clone(),
        source_checkpoint_sha256: checkpoint.checkpoint_sha256.clone(),
        source_provider_lock_sha256: stage.provider_lock.lock_sha256.clone(),
        source_implementation_sha256: stage
            .provider_lock
            .payload
            .implementation
            .executable_sha256
            .clone(),
        source_configuration_sha256: stage
            .provider_lock
            .payload
            .effective_configuration_sha256
            .clone(),
        source_provider: stage.provider_lock.payload.provider.clone(),
        duration_tolerance_milliseconds: selection.duration_tolerance_milliseconds,
        timing_basis: AudioStemTimingBasis::ZeroOriginDurationOnly,
    };
    lineage.validate()?;
    Ok(VerifiedStemImport {
        selected_path,
        evidence_path,
        lineage,
        authority_bindings: authority.bindings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(frames: u64, rate: u32) -> AudioSource {
        AudioSource {
            artifact: AudioArtifactReference {
                id: "audio".to_owned(),
                sha256: "a".repeat(64),
                byte_size: 100,
            },
            stream_index: 0,
            sample_rate_hz: rate,
            channels: 2,
            frame_count: frames,
            origin: AudioRationalTime {
                numerator: 0,
                denominator: 1,
            },
            stem: None,
        }
    }

    #[test]
    fn duration_tolerance_is_inclusive_and_exact_across_clocks() {
        let mix = source(48_000, 48_000);
        assert!(verify_duration(&mix, &source(44_100, 44_100), 0).is_ok());
        assert!(verify_duration(&mix, &source(44_982, 44_100), 20).is_ok());
        assert!(verify_duration(&mix, &source(44_983, 44_100), 20).is_err());
        assert!(verify_duration(&mix, &source(43_218, 44_100), 20).is_ok());
        assert!(verify_duration(&mix, &source(43_217, 44_100), 20).is_err());
    }

    #[test]
    fn confined_authority_rejects_parent_and_leaf_symlinks() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        fs::create_dir(root.join("real")).unwrap();
        fs::write(root.join("real/evidence.json"), b"{}").unwrap();
        assert!(confined_path(root, "real/evidence.json").is_ok());
        for path in [
            "../evidence.json",
            "/evidence.json",
            "real/./evidence.json",
            "real//evidence.json",
        ] {
            assert!(confined_path(root, path).is_err());
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("real"), root.join("alias")).unwrap();
            std::os::unix::fs::symlink(root.join("real/evidence.json"), root.join("leaf.json"))
                .unwrap();
            assert!(confined_path(root, "alias/evidence.json").is_err());
            assert!(confined_path(root, "leaf.json").is_err());
        }
    }
}
