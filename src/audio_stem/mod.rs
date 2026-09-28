//! Verified whole-stem lineage composed with the existing audio Pipeline v3 stages.
//!
//! Historical separation outputs are imported without running their provider.
//! The analysis clock remains the selected stem's own zero-origin sample grid.

mod importer;
mod provider;
mod types;

use std::path::Path;

pub(crate) use importer::{VerifiedStemImport, import_selection};
pub use types::*;

use crate::{
    ArtifactRole, AuthoredPipelineInput, ComponentInventory, Error, ErrorCategory,
    PipelineInputBinding, PipelineV3Configuration, PipelineV3Plan, ProviderInvocationRequest,
    ProviderManifest, ProviderRegistration, ProviderRegistry, Result,
};

const MANIFEST: &[u8] = include_bytes!("../../providers/audio-stem/manifest.json");
const PIPELINE: &[u8] = include_bytes!("../../providers/audio-stem/pipeline.yml");

pub(crate) fn augment_pipeline(
    pipeline: &mut PipelineV3Configuration,
    registry: &mut ProviderRegistry,
    bindings: &mut Vec<PipelineInputBinding>,
    original_mix: &Path,
    executable: &Path,
    imported: &VerifiedStemImport,
) -> Result<()> {
    let analysis = pipeline
        .outputs
        .iter()
        .find(|output| output.id == "analysis")
        .ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                "audio pipeline has no normalized analysis output",
            )
        })?;
    let analysis_artifact_id = analysis.artifact.clone();
    let dependency = pipeline
        .stages
        .iter()
        .find(|stage| {
            stage.outputs.iter().any(|binding| {
                binding
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.id == analysis_artifact_id)
            })
        })
        .ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                "audio analysis producer is missing",
            )
        })?
        .id
        .clone();
    let configuration = AudioStemProviderConfiguration {
        schema: AUDIO_STEM_CONFIGURATION_SCHEMA_V1.to_owned(),
        lineage: imported.lineage.clone(),
        upstream_analysis_artifact_id: analysis_artifact_id.clone(),
    }
    .provider_configuration()?;
    registry.register(ProviderRegistration::new(
        "audio-stem-native",
        ProviderManifest::from_json_slice(MANIFEST)?,
        configuration,
        executable,
        "aniflow-audio-stem-v1",
        ComponentInventory {
            tools: Vec::new(),
            codecs: Vec::new(),
            models: Vec::new(),
        },
    )?)?;
    let mut extension = PipelineV3Configuration::from_yaml_slice(PIPELINE)
        .map_err(|error| Error::new(ErrorCategory::Internal, error.message))?;
    if extension.stages.len() != 1 {
        return Err(Error::new(
            ErrorCategory::Internal,
            "stem pipeline extension must contain exactly one stage",
        ));
    }
    let mut stage = extension.stages.remove(0);
    stage.depends_on = vec![dependency];
    for input in &mut stage.inputs {
        if input.port == "analysis" {
            input.artifacts = vec![analysis_artifact_id.clone()];
        } else if input.port == "authority" {
            input.artifacts = imported
                .authority_bindings
                .iter()
                .map(|binding| binding.artifact_id.clone())
                .collect();
        }
    }
    pipeline.inputs.extend(
        extension
            .inputs
            .into_iter()
            .filter(|input| input.id == "original_mix" || input.id == "separation_evidence"),
    );
    for binding in &imported.authority_bindings {
        pipeline.inputs.push(AuthoredPipelineInput {
            id: binding.artifact_id.clone(),
            artifact_type: "application/json".to_owned(),
            artifact_role: ArtifactRole::ValidationEvidence,
            stream_role: None,
        });
    }
    bindings.push(PipelineInputBinding::new("original_mix", original_mix));
    bindings.push(PipelineInputBinding::new(
        "separation_evidence",
        &imported.evidence_path,
    ));
    bindings.extend(imported.authority_bindings.clone());
    pipeline.stages.push(stage);
    pipeline.outputs.retain(|output| output.id != "analysis");
    pipeline.outputs.extend(extension.outputs);
    Ok(())
}

pub(crate) fn execute_provider_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    provider::execute_invocation(request)
}

pub(crate) fn bind_authorities_plan(
    plan: &PipelineV3Plan,
    imported: Option<&VerifiedStemImport>,
) -> Result<()> {
    if let Some(imported) = imported {
        for reference in [
            &imported.lineage.original_mix.artifact,
            &imported.lineage.selected_stem.artifact,
            &imported.lineage.relationship_evidence,
        ]
        .into_iter()
        .chain(&imported.lineage.authority_artifacts)
        {
            crate::audio_inspection::bind_source_plan(plan, reference)?;
        }
    }
    Ok(())
}
