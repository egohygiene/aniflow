//! Final lineage attachment, using only independently retained immutable evidence.
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::types::*;
use crate::audio_analysis::*;
use crate::audio_inspection::{process::hash_regular, wav};
use crate::{
    ArtifactKind, ArtifactRole, CapabilityReference, Error, ErrorCategory,
    ProviderInvocationArtifactBinding, ProviderInvocationRequest, ProviderReference, Result,
    StreamRole,
};

fn fail(message: &str) -> Error {
    Error::new(ErrorCategory::Media, message)
}

fn binding<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> Result<&'a ProviderInvocationArtifactBinding> {
    let mut values = request.inputs.iter().filter(|binding| binding.port == port);
    let value = values
        .next()
        .ok_or_else(|| fail("stem input binding is missing"))?;
    if values.next().is_some() {
        return Err(fail("stem input binding is ambiguous"));
    }
    Ok(value)
}

fn verify_inputs(
    request: &ProviderInvocationRequest,
    config: &AudioStemProviderConfiguration,
) -> Result<()> {
    let expected_count = 4 + config.lineage.authority_artifacts.len();
    if request.inputs.len() != expected_count || request.outputs.len() != 2 {
        return Err(fail(
            "stem provider input/output cardinality differs from its declared profile",
        ));
    }
    for (port, id, mime, role, stream) in [
        (
            "audio",
            "source_audio",
            "audio/wav",
            ArtifactRole::TemporalSource,
            Some(StreamRole::Audio),
        ),
        (
            "original_mix",
            "original_mix",
            "audio/wav",
            ArtifactRole::TemporalSource,
            Some(StreamRole::Audio),
        ),
        (
            "separation_evidence",
            "separation_evidence",
            "application/octet-stream",
            ArtifactRole::ValidationEvidence,
            None,
        ),
        (
            "analysis",
            config.upstream_analysis_artifact_id.as_str(),
            "application/vnd.aniflow.audio-analysis+json",
            ArtifactRole::ValidationEvidence,
            Some(StreamRole::TimedMetadata),
        ),
    ] {
        let input = binding(request, port)?;
        if input.artifact_id != id
            || input.artifact_type != mime
            || input.artifact_role != role
            || input.stream_role != stream
            || input.kind != ArtifactKind::File
        {
            return Err(fail(
                "stem input identity, role, or type differs from the declared profile",
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for input in request
        .inputs
        .iter()
        .filter(|input| input.port == "authority")
    {
        if input.artifact_type != "application/json"
            || input.artifact_role != ArtifactRole::ValidationEvidence
            || input.stream_role.is_some()
            || input.kind != ArtifactKind::File
            || !ids.insert(input.artifact_id.as_str())
            || !config
                .lineage
                .authority_artifacts
                .iter()
                .any(|reference| reference.id == input.artifact_id)
        {
            return Err(fail(
                "stem authority input differs from its exact retained identity",
            ));
        }
    }
    if ids.len() != config.lineage.authority_artifacts.len() {
        return Err(fail("stem authority inputs are incomplete"));
    }
    for (port, id, mime) in [
        (
            "lineage",
            "stem_lineage",
            "application/vnd.aniflow.audio-stem-lineage+json",
        ),
        (
            "analysis",
            "stem_analysis",
            "application/vnd.aniflow.audio-analysis+json",
        ),
    ] {
        let outputs = request
            .outputs
            .iter()
            .filter(|output| output.port == port)
            .collect::<Vec<_>>();
        if outputs.len() != 1 {
            return Err(fail("stem output binding is missing or ambiguous"));
        }
        let output = outputs[0];
        if output.artifact_id != id
            || output.artifact_type != mime
            || output.artifact_role != ArtifactRole::ValidationEvidence
            || output.stream_role != Some(StreamRole::TimedMetadata)
            || output.kind != ArtifactKind::File
        {
            return Err(fail(
                "stem output identity, role, or type differs from its declared profile",
            ));
        }
    }
    Ok(())
}

fn verify_file(path: &Path, reference: &AudioArtifactReference) -> Result<()> {
    if path.canonicalize().ok().as_deref() != Some(path) {
        return Err(fail(
            "stem retained evidence path must remain canonical and symlink-free",
        ));
    }
    let (sha256, size) = hash_regular(path, reference.byte_size).map_err(|_| {
        fail("stem evidence is unavailable, nonregular, or exceeds its recorded bound")
    })?;
    if sha256 != reference.sha256 || size != reference.byte_size {
        return Err(fail(
            "stem evidence differs from the exact planned digest or size",
        ));
    }
    Ok(())
}

fn verify_retained(
    request: &ProviderInvocationRequest,
    config: &AudioStemProviderConfiguration,
) -> Result<()> {
    for reference in [
        &config.lineage.selected_stem.artifact,
        &config.lineage.original_mix.artifact,
        &config.lineage.relationship_evidence,
    ]
    .into_iter()
    .chain(&config.lineage.authority_artifacts)
    {
        let input = request
            .inputs
            .iter()
            .find(|input| input.artifact_id == reference.id)
            .ok_or_else(|| fail("stem retained evidence input is missing"))?;
        verify_file(&input.path, reference)?;
    }
    Ok(())
}

fn read_analysis(path: &Path, id: &str) -> Result<(AudioAnalysis, AudioArtifactReference)> {
    if path.canonicalize().ok().as_deref() != Some(path) {
        return Err(fail(
            "upstream audio evidence path must be canonical and symlink-free",
        ));
    }
    let maximum = 8 * 1024 * 1024;
    let (sha256, byte_size) = hash_regular(path, maximum)
        .map_err(|_| fail("upstream audio analysis must be a bounded regular artifact"))?;
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| fail("upstream audio analysis cannot be opened"))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("upstream audio analysis cannot be read"))?;
    if bytes.len() as u64 != byte_size || format!("{:x}", Sha256::digest(&bytes)) != sha256 {
        return Err(fail("upstream audio analysis changed while reading"));
    }
    Ok((
        AudioAnalysis::from_json_slice(&bytes)?,
        AudioArtifactReference {
            id: id.to_owned(),
            sha256,
            byte_size,
        },
    ))
}

fn inspect_clock(path: &Path, expected: &AudioSource) -> Result<()> {
    let wave = wav::inspect(path)?;
    if wave.sample_rate_hz != expected.sample_rate_hz
        || wave.channels != expected.channels
        || wave.frame_count != expected.frame_count
    {
        return Err(fail("stem PCM clock differs from its verified lineage"));
    }
    Ok(())
}

pub(super) fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    request.validate()?;
    let config: AudioStemProviderConfiguration = serde_json::from_value(
        serde_json::to_value(&request.configuration.values)
            .map_err(|_| fail("cannot read stem provider configuration"))?,
    )
    .map_err(|_| fail("invalid closed stem provider configuration"))?;
    config.validate()?;
    if request.configuration != config.provider_configuration()? {
        return Err(fail(
            "stem invocation must select its exact provider configuration",
        ));
    }
    verify_inputs(request, &config)?;
    verify_retained(request, &config)?;
    inspect_clock(
        &binding(request, "audio")?.path,
        &config.lineage.selected_stem,
    )?;
    inspect_clock(
        &binding(request, "original_mix")?.path,
        &config.lineage.original_mix,
    )?;
    let analysis_input = binding(request, "analysis")?;
    let (mut analysis, upstream_analysis_artifact) =
        read_analysis(&analysis_input.path, &config.upstream_analysis_artifact_id)?;
    let mut expected_source = config.lineage.selected_stem.clone();
    expected_source.stem = None;
    if analysis.source != expected_source
        || !matches!(
            analysis.status,
            AudioAnalysisStatus::Complete | AudioAnalysisStatus::Partial
        )
    {
        return Err(fail(
            "upstream audio analysis does not describe the selected whole stem",
        ));
    }
    let implementation_sha256 = hash_regular(
        &std::env::current_exe()
            .map_err(|_| fail("stem implementation identity is unavailable"))?,
        512 * 1024 * 1024,
    )
    .map_err(|_| fail("stem implementation identity cannot be observed"))?
    .0;
    let report = AudioStemLineageReport {
        schema: AUDIO_STEM_LINEAGE_SCHEMA_V1.to_owned(),
        lineage: config.lineage.clone(),
        upstream_analysis_artifact: upstream_analysis_artifact.clone(),
        provider: ProviderReference {
            id: AUDIO_STEM_PROVIDER_ID.to_owned(),
            version: AUDIO_STEM_PROVIDER_VERSION.to_owned(),
        },
        implementation_sha256,
        configuration_sha256: request.configuration.effective_configuration_sha256.clone(),
        provider_lock_sha256: request.provider_lock_sha256.clone(),
    };
    let lineage_bytes = report.canonical_json_bytes()?;
    let lineage_reference = AudioArtifactReference {
        id: "stem_lineage".to_owned(),
        sha256: format!("{:x}", Sha256::digest(&lineage_bytes)),
        byte_size: lineage_bytes.len() as u64,
    };
    attach_lineage(&mut analysis, &report, lineage_reference)?;
    let analysis_bytes = analysis.canonical_json_bytes()?;
    if analysis_bytes.len() > 8 * 1024 * 1024 || lineage_bytes.len() > 8 * 1024 * 1024 {
        return Err(fail("stem evidence exceeds its bounded report profile"));
    }
    verify_retained(request, &config)?;
    verify_file(&analysis_input.path, &upstream_analysis_artifact)?;
    let lineage_output = request
        .outputs
        .iter()
        .find(|output| output.port == "lineage")
        .expect("verified output");
    let analysis_output = request
        .outputs
        .iter()
        .find(|output| output.port == "analysis")
        .expect("verified output");
    let parent = lineage_output
        .path
        .parent()
        .ok_or_else(|| fail("stem output parent is missing"))?;
    if analysis_output.path.parent() != Some(parent) {
        return Err(fail(
            "stem artifacts require the same assigned output directory",
        ));
    }
    prepare_output(parent)?;
    publish_new(&lineage_output.path, &lineage_bytes)?;
    publish_new(&analysis_output.path, &analysis_bytes)
}

fn attach_lineage(
    analysis: &mut AudioAnalysis,
    report: &AudioStemLineageReport,
    lineage_reference: AudioArtifactReference,
) -> Result<()> {
    let provider_id = "stem-lineage-provider";
    analysis.source = report.lineage.selected_stem.clone();
    for observation in &mut analysis.observations {
        observation.scope.stem_id = Some(report.lineage.stem_id.clone());
    }
    for timeline in &mut analysis.timelines {
        timeline.scope.stem_id = Some(report.lineage.stem_id.clone());
    }
    for excerpt in &mut analysis.excerpts {
        excerpt.scope.stem_id = Some(report.lineage.stem_id.clone());
    }
    for reference in [
        report.upstream_analysis_artifact.clone(),
        report.lineage.relationship_evidence.clone(),
        lineage_reference,
    ]
    .into_iter()
    .chain(report.lineage.authority_artifacts.iter().cloned())
    {
        if analysis
            .artifacts
            .iter()
            .any(|artifact| artifact.id == reference.id)
        {
            return Err(fail(
                "stem lineage artifact identity collides with upstream audio evidence",
            ));
        }
        analysis.artifacts.push(reference);
    }
    analysis.providers.push(AudioProviderEvidence {
        id: provider_id.to_owned(),
        provider: report.provider.clone(),
        implementation_sha256: report.implementation_sha256.clone(),
        configuration_sha256: report.configuration_sha256.clone(),
        tools: Vec::new(),
        models: AudioModelEvidence::NoneRequired {},
        license: AudioLicenseEvidence::Unavailable {
            reason: "license evidence is not collected by this lineage attachment".to_owned(),
        },
    });
    if analysis
        .capabilities
        .iter()
        .any(|value| value.capability.id == AUDIO_STEM_CAPABILITY_ID)
    {
        return Err(fail(
            "upstream audio evidence already declares a stem lineage capability",
        ));
    }
    let outcome = AudioCapabilityOutcome {
        capability: CapabilityReference {
            id: AUDIO_STEM_CAPABILITY_ID.to_owned(),
            version: "1.0.0".to_owned(),
        },
        status: AudioAnalysisStatus::Complete,
        provider_evidence_ids: vec![provider_id.to_owned()],
        evidence_artifact_ids: vec![
            "source_audio".to_owned(),
            "original_mix".to_owned(),
            "separation_evidence".to_owned(),
            "stem_lineage".to_owned(),
        ],
        diagnostic_ids: Vec::new(),
    };
    analysis.capabilities.push(outcome);
    analysis.validate()
}

fn prepare_output(parent: &Path) -> Result<()> {
    if !parent.is_absolute() {
        return Err(fail("stem output directory must be absolute"));
    }
    let mut existing = parent;
    while !existing.exists() {
        if existing.is_symlink() {
            return Err(fail("stem output directory cannot contain symlinks"));
        }
        existing = existing
            .parent()
            .ok_or_else(|| fail("stem output ancestor is missing"))?;
    }
    if existing.canonicalize().ok().as_deref() != Some(existing) {
        return Err(fail(
            "stem output directory must be canonical and symlink-free",
        ));
    }
    std::fs::create_dir_all(parent)
        .map_err(|_| fail("cannot create assigned stem output directory"))
}

fn publish_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| fail("cannot exclusively create stem evidence artifact"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| fail("cannot publish complete stem evidence"))
}
