//! Explicit manual frame repairs after a fresh import in one ordered v3 run.
mod provider;
mod types;
pub mod allenk;

use sha2::{Digest, Sha256};
use crate::audio_inspection::process::verify_pin;
use crate::frame_import::{self, FrameImportRequest};
use crate::frame_selection::{plan_from_rife_index, FrameSelectionPlan};
use crate::frame_io::identity;
use crate::provider::decode_json;
use crate::{CancellationToken, Error, ErrorCategory, Result};
pub use provider::execute_invocation;
pub use types::*;

pub(crate) const MAXIMUM_TOOL_BYTES: u64 = 134_217_728;
fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input, message) }

pub(crate) fn validate_working_bytes(maximum_working_bytes: u64) -> Result<()> {
    if !(1_048_576..=17_592_186_044_416).contains(&maximum_working_bytes) {
        return Err(invalid("unsupported frame repair working-byte bound"));
    }
    Ok(())
}

impl FrameRepairConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("frame repair configuration exceeds 64 KiB")); }
        let value: Self = decode_json(bytes, "frame repair configuration")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        self.import.validate()?;
        if self.schema != FRAME_REPAIR_CONFIGURATION_SCHEMA_V1
        { return Err(invalid("unsupported frame repair configuration or working-byte bound")); }
        validate_working_bytes(self.maximum_working_bytes)?;
        let profile = self.processor.profile.as_bytes();
        if profile.is_empty() || profile.len() > 128 || !profile[0].is_ascii_alphanumeric()
            || profile.iter().any(|byte| !byte.is_ascii_alphanumeric() && !b"._-".contains(byte))
        { return Err(invalid("direct-image profile must be an explicit bounded ASCII token")); }
        let mut limits = self.import.tool_limits();
        limits.ffprobe = self.processor.tool.clone();
        limits.validate()?;
        let args = &self.processor.arguments;
        let mut bytes = 0_usize;
        if args.is_empty() || args.len() > 256 { return Err(invalid("direct-image arguments require 1 to 256 literal arguments")); }
        for arg in args {
            bytes = bytes.checked_add(arg.len()).ok_or_else(|| invalid("argument size overflow"))?;
            if arg.is_empty() || arg.len() > 4096 || bytes > 32_768 || arg.chars().any(char::is_control)
                || ((arg.contains('{') || arg.contains('}')) && arg != "{input}" && arg != "{output}")
            { return Err(invalid("direct-image argv exceeds its bounds or contains an ambiguous placeholder")); }
        }
        if args.iter().filter(|arg| arg.as_str() == "{input}").count() != 1
            || args.iter().filter(|arg| arg.as_str() == "{output}").count() != 1
        { return Err(invalid("direct-image argv requires exactly one whole {input} and {output} argument")); }
        Ok(())
    }
}

impl FrameRepairOperation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        if self.schema != FRAME_REPAIR_PROVIDER_CONFIGURATION_SCHEMA_V1
            || self.ranges.is_empty() || self.ranges.len() > crate::frame_selection::MAXIMUM_FRAME_SELECTION_RANGES
            || self.ranges.iter().any(|range| range.first == 0 || range.last < range.first)
        { return Err(invalid("frame repair requires explicit bounded inclusive ranges")); }
        Ok(())
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let values = serde_json::to_value(self).map_err(|error| invalid(error.to_string()))?
            .as_object().ok_or_else(|| invalid("frame repair operation must be an object"))?
            .iter().map(|(key, value)| (key.clone(), value.clone())).collect();
        crate::ProviderConfiguration::new(
            crate::ProviderReference { id: FRAME_REPAIR_PROVIDER_ID.to_owned(), version: "1.0.0".to_owned() },
            crate::CapabilityReference { id: FRAME_REPAIR_CAPABILITY_ID.to_owned(), version: "1.0.0".to_owned() },
            configuration_schema_reference(), values,
        )
    }

    /// Inert wrapper authority. Generic tool version/profile remain declarations.
    pub fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let schema = configuration_schema_reference();
        let tools: Vec<_> = [("ffmpeg", &self.settings.import.ffmpeg), ("direct-image-processor", &self.settings.processor.tool)]
            .into_iter().map(|(id, pin)| serde_json::json!({"id":id,"version_requirement":format!("={}",pin.version),"sha256":pin.sha256})).collect();
        let value = serde_json::json!({
            "schema":"aniflow.provider-manifest/v1",
            "provider":{"id":FRAME_REPAIR_PROVIDER_ID,"version":"1.0.0","display_name":"aniflow explicit selective frame repair"},
            "configuration_schemas":[schema.clone()],
            "capabilities":[{
                "id":FRAME_REPAIR_CAPABILITY_ID,"version":"1.0.0","kind":"frame_processor","configuration_schema":schema,
                "inputs":[
                    {"name":"frames","artifact_type":"application/vnd.aniflow.rife-frames","artifact_role":"intermediate","stream_role":"video","cardinality":"one","immutable":true},
                    {"name":"import","artifact_type":"application/vnd.aniflow.frame-import-report+json","artifact_role":"validation_evidence","stream_role":"timed_metadata","cardinality":"one","immutable":true},
                    {"name":"index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata","cardinality":"one","immutable":true}
                ],
                "outputs":[
                    {"name":"frames","artifact_type":"application/vnd.aniflow.rife-frames","artifact_role":"intermediate","stream_role":"video","cardinality":"one","immutable":true},
                    {"name":"index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata","cardinality":"one","immutable":true},
                    {"name":"evidence","artifact_type":"application/vnd.aniflow.frame-repair-report+json","artifact_role":"validation_evidence","stream_role":"timed_metadata","cardinality":"one","immutable":true}
                ],
                "batching":"whole_artifact","max_concurrency":1,
                "requirements":{"tools":tools,"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
                "behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},
                "lifecycle":{"progress":"none","cancellation":"process_signal"}
            }],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}
        });
        crate::ProviderManifest::from_json_slice(&serde_json::to_vec(&value).map_err(|error| invalid(error.to_string()))?)
    }
}

pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference {
        id: FRAME_REPAIR_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(), version:"1.0.0".to_owned(),
        sha256:format!("{:x}", Sha256::digest(include_bytes!("../../providers/frame-repair/provider-configuration.schema.json"))),
    }
}

fn import_request(request: &FrameRepairRequest) -> FrameImportRequest {
    FrameImportRequest::new(&request.source_video, &request.frames_directory, &request.frame_index,
        request.configuration.import.clone(), &request.provider_executable)
}

fn selected(selection: &FrameSelectionPlan, ordinal: u64) -> bool {
    selection.ranges.iter().any(|range| ordinal >= range.start_ordinal && ordinal < range.end_ordinal_exclusive)
}

// Includes existing source bytes, original frames, candidate plus accepted
// import/repair copies, one bounded frame pair, tool copies and evidence bounds.
// This is admission accounting, not a filesystem reservation or an OS quota.
fn admit_working_bytes(config: &FrameRepairConfiguration, source_bytes: u64, input_bytes: u64, unselected_bytes: u64, selection: &FrameSelectionPlan, tool_bytes: u64) -> Result<()> {
    let checked = || -> Option<u64> {
        let repaired = selection.selected_frame_count.checked_mul(config.import.maximum_frame_bytes)?.checked_add(unselected_bytes)?;
        source_bytes.checked_add(input_bytes.checked_mul(3)?)?
            .checked_add(repaired.checked_mul(2)?)?
            .checked_add(config.import.maximum_frame_bytes.checked_mul(2)?)?
            .checked_add(tool_bytes.checked_mul(2)?)?
            .checked_add(6 * FRAME_REPAIR_MAXIMUM_REPORT_BYTES)?
            .checked_add(4 * crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES)
    };
    if checked().is_none_or(|bytes| bytes > config.maximum_working_bytes) {
        return Err(invalid("import, repair, accepted copies and scratch exceed maximum_working_bytes"));
    }
    Ok(())
}

fn prepare(request: &FrameRepairRequest, cancellation: &CancellationToken) -> Result<(crate::PipelineV3Plan, crate::ProviderRegistry, Vec<crate::PipelineInputBinding>)> {
    prepare_with_named(request, None, cancellation)
}

fn prepare_with_named(request: &FrameRepairRequest, named: Option<&allenk::AllenkRepairConfiguration>, cancellation: &CancellationToken) -> Result<(crate::PipelineV3Plan, crate::ProviderRegistry, Vec<crate::PipelineInputBinding>)> {
    request.configuration.validate()?;
    request.execution_limits.validate()?;
    let mut prepared = frame_import::prepare_inputs(&import_request(request), cancellation)?;
    let selection = plan_from_rife_index(&prepared.index, &request.ranges)?;
    let mut unselected_bytes = 0_u64;
    for frame in &prepared.index.frames {
        if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution, "frame repair planning cancelled")); }
        if !selected(&selection, frame.ordinal) {
            let observed = identity(&request.frames_directory.join(&frame.filename), request.configuration.import.maximum_frame_bytes)?;
            if observed.0 != frame.sha256 { return Err(invalid("unselected frame changed during repair planning")); }
            unselected_bytes = unselected_bytes.checked_add(observed.1).ok_or_else(|| invalid("frame byte count overflow"))?;
        }
    }
    let tools = [(&request.configuration.import.ffmpeg, "ffmpeg"), (&request.configuration.processor.tool, "direct-image-processor")];
    let mut tool_bytes = 0_u64;
    for (pin, id) in tools {
        verify_pin(pin, id).map_err(|_| Error::new(ErrorCategory::Dependency, "frame repair executable pin is unavailable or changed"))?;
        tool_bytes += identity(&pin.executable, MAXIMUM_TOOL_BYTES)?.1;
    }
    admit_working_bytes(&request.configuration, prepared.source.1, prepared.frame_bytes, unselected_bytes, &selection, tool_bytes)?;
    let ranges = selection.ranges.iter().map(|range| crate::frame_selection::InclusiveFrameRange { first:range.first_frame, last:range.last_frame }).collect();
    let operation = FrameRepairOperation { schema:FRAME_REPAIR_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(), settings:request.configuration.clone(), ranges };
    let (manifest, configuration, implementation) = if let Some(settings) = named {
        let named_operation = allenk::AllenkRepairOperation { schema:allenk::ALLENK_REPAIR_OPERATION_SCHEMA_V1.to_owned(),settings:settings.clone(),ranges:operation.ranges.clone() };
        (named_operation.manifest()?,named_operation.provider_configuration()?,"aniflow-frame-repair-allenk-v1")
    } else { (operation.manifest()?,operation.provider_configuration()?,"aniflow-frame-repair-v1") };
    prepared.registry.register(crate::ProviderRegistration::new("frame-repair-native", manifest, configuration, request.provider_executable.clone(), implementation,
        crate::ComponentInventory { tools:tools.into_iter().map(|(pin,id)| crate::ComponentIdentity { id:id.to_owned(), version:pin.version.clone(), sha256:Some(pin.sha256.clone()) }).collect(), codecs:Vec::new(), models:Vec::new() })?)?;
    let mut pipeline = crate::PipelineV3Configuration::from_yaml_slice(include_bytes!("../../providers/frame-repair/pipeline.yml"))
        .map_err(|failure| Error::new(failure.category(), failure.message))?;
    if named.is_some() {
        pipeline.name = "allenk-selective-frame-repair".to_owned();
        let repair = pipeline.stages.iter_mut().find(|stage| stage.id == "repair_frames").ok_or_else(|| invalid("fixed repair pipeline is missing its repair stage"))?;
        repair.capability.id = allenk::ALLENK_REPAIR_CAPABILITY_ID.to_owned();
    }
    let plan = crate::resolve_pipeline_v3(&pipeline, &prepared.bindings, &prepared.registry, &crate::audio_inspection::context())
        .map_err(|failure| Error::new(failure.category(), failure.message))?;
    frame_import::bind_prepared_plan(&plan, &prepared)?;
    if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution, "frame repair planning cancelled")); }
    Ok((plan, prepared.registry, prepared.bindings))
}

/// Resolve both ordered stages without launching either tool or writing a run.
pub fn plan(request: &FrameRepairRequest, cancellation: &CancellationToken) -> Result<crate::PipelineV3Plan> { Ok(prepare(request, cancellation)?.0) }

/// Import and repair into a new run; no prior standalone report is trusted as
/// an accepted checkpoint. Existing v3 runtime owns stage acceptance/cancellation.
pub fn run<F>(request: FrameRepairRequest, output_directory: Option<std::path::PathBuf>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let parent = frame_import::output_parent(&import_request(&request), output_directory)?;
    if parent.starts_with(&request.configuration.processor.tool.executable) { return Err(invalid("repair output parent overlaps the processor executable")); }
    let (plan, registry, bindings) = prepare(&request, cancellation)?;
    let mut execution = crate::PipelineV3RunRequest::new(plan, bindings, registry).with_execution_limits(request.execution_limits);
    execution.output_directory = Some(parent);
    crate::run_v3_with_progress_and_cancellation(execution, cancellation, on_progress)
}
