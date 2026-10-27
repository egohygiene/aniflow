//! Named basic-image repair with explicit embedded profile selection.
//!
//! The executable binds compiled profile assets. Host shared libraries remain
//! unobserved; run-local reuse is not cross-host reproducibility qualification.
use std::io::Read;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::audio_inspection::AudioToolPin;
use crate::audio_inspection::process::{GroupPolicy, open_regular, run_tool_isolated, verify_pin};
use crate::frame_import::FrameImportConfiguration;
use crate::frame_selection::InclusiveFrameRange;
use crate::frame_io::identity;
use crate::provider::{canonical_json_bytes, decode_json};
use crate::{CancellationToken, Error, ErrorCategory, Result};
use super::{FrameRepairConfiguration, FrameRepairOperation, FrameRepairReport, FrameRepairRequest, DirectImageProcessor};

pub const ALLENK_REPAIR_CONFIGURATION_SCHEMA_V1: &str = "aniflow.allenk-frame-repair-configuration/v1";
pub const ALLENK_REPAIR_OPERATION_SCHEMA_V1: &str = "aniflow.allenk-frame-repair-provider-configuration/v1";
pub const ALLENK_REPAIR_REPORT_SCHEMA_V1: &str = "aniflow.allenk-frame-repair-report/v1";
pub const ALLENK_REPAIR_PROVIDER_ID: &str = "org.egohygiene.aniflow.frame-repair-allenk";
pub const ALLENK_REPAIR_CAPABILITY_ID: &str = "aniflow/frame-repair-allenk";
pub const ALLENK_INTERFACE_REVISION: &str = "7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e";
pub const ALLENK_INTERFACE_VERSION: &str = "0.3.2";
const MAXIMUM_PLAN_BYTES: u64 = 67_108_864;

fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input, message) }

/// Visible-overlay family, selected explicitly without automatic fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllenkWatermarkProfile { CurrentV2, LegacyV1 }

/// Closed, source-derived basic image interface. No arbitrary argv or external
/// models are admitted; optional denoising and advanced-region paths are absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllenkRepairConfiguration {
    pub schema: String,
    pub import: FrameImportConfiguration,
    pub tool: AudioToolPin,
    pub watermark_profile: AllenkWatermarkProfile,
    #[serde(default = "threshold")]
    pub detection_threshold_percent: u8,
    #[serde(default = "working_bytes")]
    pub maximum_working_bytes: u64,
}
fn threshold() -> u8 { 25 }
fn working_bytes() -> u64 { 274_877_906_944 }

impl AllenkRepairConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("allenk repair configuration exceeds 64 KiB")); }
        let configuration: Self = decode_json(bytes, "allenk repair configuration")?;
        configuration.validate()?;
        Ok(configuration)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != ALLENK_REPAIR_CONFIGURATION_SCHEMA_V1
            || self.tool.version != ALLENK_INTERFACE_VERSION || self.detection_threshold_percent > 100 {
            return Err(invalid("allenk repair requires the pinned 0.3.2 basic-image interface and a threshold from 0 to 100 percent"));
        }
        self.normalized().validate()
    }

    /// Fixed basic argv. Omitting denoise is intentional: even `--denoise off`
    /// selects a different advanced pipeline in the pinned upstream interface.
    fn arguments(&self) -> Vec<String> {
        let profile = match self.watermark_profile { AllenkWatermarkProfile::CurrentV2 => "--no-legacy", AllenkWatermarkProfile::LegacyV1 => "--legacy" };
        vec!["--input".to_owned(), "{input}".to_owned(), "--output".to_owned(), "{output}".to_owned(),
            "--remove".to_owned(), "--no-banner".to_owned(), profile.to_owned(), "--threshold".to_owned(),
            format!("{}.{:02}", self.detection_threshold_percent / 100, self.detection_threshold_percent % 100)]
    }

    pub(super) fn normalized(&self) -> FrameRepairConfiguration {
        FrameRepairConfiguration {
            schema: super::FRAME_REPAIR_CONFIGURATION_SCHEMA_V1.to_owned(), import: self.import.clone(),
            processor: DirectImageProcessor {
                profile: match self.watermark_profile { AllenkWatermarkProfile::CurrentV2 => "allenk-current-v2", AllenkWatermarkProfile::LegacyV1 => "allenk-legacy-v1" }.to_owned(),
                tool: self.tool.clone(), arguments: self.arguments(),
            }, maximum_working_bytes: self.maximum_working_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllenkRepairOperation {
    pub schema: String,
    pub settings: AllenkRepairConfiguration,
    pub ranges: Vec<InclusiveFrameRange>,
}

impl AllenkRepairOperation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        if self.schema != ALLENK_REPAIR_OPERATION_SCHEMA_V1 { return Err(invalid("unsupported allenk repair operation")); }
        self.normalized().validate()
    }
    pub(super) fn normalized(&self) -> FrameRepairOperation {
        FrameRepairOperation { schema:super::FRAME_REPAIR_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(), settings:self.settings.normalized(), ranges:self.ranges.clone() }
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let values = serde_json::to_value(self).map_err(|error| invalid(error.to_string()))?
            .as_object().ok_or_else(|| invalid("allenk operation must be an object"))?
            .iter().map(|(key,value)| (key.clone(),value.clone())).collect();
        crate::ProviderConfiguration::new(
            crate::ProviderReference { id:ALLENK_REPAIR_PROVIDER_ID.to_owned(),version:"1.0.0".to_owned() },
            crate::CapabilityReference { id:ALLENK_REPAIR_CAPABILITY_ID.to_owned(),version:"1.0.0".to_owned() },
            configuration_schema_reference(), values,
        )
    }
    pub fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        // Retain the same import/repair artifact contract and execution owner.
        let mut value = serde_json::to_value(self.normalized().manifest()?).map_err(|error| invalid(error.to_string()))?;
        value["provider"]["id"] = ALLENK_REPAIR_PROVIDER_ID.into();
        value["provider"]["display_name"] = "aniflow explicit allenk frame repair".into();
        let schema = serde_json::to_value(configuration_schema_reference()).map_err(|error| invalid(error.to_string()))?;
        value["configuration_schemas"] = serde_json::json!([schema.clone()]);
        value["capabilities"][0]["id"] = ALLENK_REPAIR_CAPABILITY_ID.into();
        value["capabilities"][0]["configuration_schema"] = schema;
        value["capabilities"][0]["outputs"][2]["artifact_type"] = "application/vnd.aniflow.allenk-frame-repair-report+json".into();
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
}

pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:ALLENK_REPAIR_OPERATION_SCHEMA_V1.to_owned(),version:"1.0.0".to_owned(),
        sha256:format!("{:x}",Sha256::digest(include_bytes!("../../providers/frame-repair-allenk/provider-configuration.schema.json"))) }
}

/// Contemporary named-profile evidence around the unchanged complete repair
/// observations. Embedded assets do not establish host-library qualification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllenkRepairReport {
    pub schema: String,
    pub upstream_revision: String,
    pub observed_processor_version: String,
    pub watermark_profile: AllenkWatermarkProfile,
    pub detection_threshold_percent: u8,
    pub profile_assets_evidence: String,
    pub denoising: String,
    pub host_runtime_evidence: String,
    pub backend_qualification: String,
    pub repair: FrameRepairReport,
}

#[derive(Debug, Clone)]
pub struct AllenkRepairRequest {
    pub source_video: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub configuration: AllenkRepairConfiguration,
    pub ranges: Vec<InclusiveFrameRange>,
    pub provider_executable: PathBuf,
    pub execution_limits: crate::ProviderExecutionLimits,
}
impl AllenkRepairRequest {
    #[must_use]
    pub fn new(source_video: impl Into<PathBuf>, frames_directory: impl Into<PathBuf>, frame_index: impl Into<PathBuf>, configuration: AllenkRepairConfiguration, ranges: Vec<InclusiveFrameRange>, provider_executable: impl Into<PathBuf>) -> Self {
        let normalized = FrameRepairRequest::new(source_video, frames_directory, frame_index, configuration.normalized(), ranges, provider_executable);
        Self { source_video:normalized.source_video,frames_directory:normalized.frames_directory,frame_index:normalized.frame_index,configuration,
            ranges:normalized.ranges,provider_executable:normalized.provider_executable,execution_limits:normalized.execution_limits }
    }
    fn normalized(&self) -> FrameRepairRequest {
        let mut request = FrameRepairRequest::new(&self.source_video,&self.frames_directory,&self.frame_index,self.configuration.normalized(),self.ranges.clone(),&self.provider_executable);
        request.execution_limits = self.execution_limits;
        request
    }
}

fn prepare(request: &AllenkRepairRequest, cancellation: &CancellationToken) -> Result<(crate::PipelineV3Plan,crate::ProviderRegistry,Vec<crate::PipelineInputBinding>)> {
    request.configuration.validate()?;
    let prepared = super::prepare_with_named(&request.normalized(),Some(&request.configuration),cancellation)?;
    if canonical_json_bytes(&prepared.0)?.len() as u64 > MAXIMUM_PLAN_BYTES { return Err(invalid("allenk repair plan exceeds 64 MiB")); }
    Ok(prepared)
}

pub fn plan(request: &AllenkRepairRequest, cancellation: &CancellationToken) -> Result<crate::PipelineV3Plan> { Ok(prepare(request,cancellation)?.0) }

pub fn run<F>(request: AllenkRepairRequest, output_directory: Option<PathBuf>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let normalized = request.normalized();
    let parent = crate::frame_import::output_parent(&super::import_request(&normalized),output_directory)?;
    if parent.starts_with(&request.configuration.tool.executable) { return Err(invalid("repair output parent overlaps the allenk executable")); }
    let (plan,registry,bindings) = prepare(&request,cancellation)?;
    crate::run_v3_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(plan,bindings,registry).with_output_directory(parent).with_execution_limits(request.execution_limits),cancellation,on_progress)
}

/// Resume the same import/whole-repair stages, never partial repair PNGs.
pub fn resume<F>(request: AllenkRepairRequest, run_directory: impl AsRef<Path>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let (plan,registry,bindings) = prepare(&request,cancellation)?;
    let root = run_directory.as_ref().canonicalize().map_err(|_| invalid("cannot resolve the allenk repair run directory"))?;
    let workspace = crate::PipelineV3Workspace::open_read_only(&root)?;
    let path = workspace.plan();
    let observed = identity(&path,MAXIMUM_PLAN_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(&path).map_err(|_| invalid("cannot open retained allenk plan"))?.take(MAXIMUM_PLAN_BYTES+1).read_to_end(&mut bytes).map_err(|_| invalid("cannot read retained allenk plan"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64) != observed { return Err(invalid("retained allenk plan changed while reading")); }
    let retained = crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|failure| Error::new(failure.category(),failure.message))?;
    if retained != plan { return Err(invalid("allenk repair source, ranges, profile or exact dependency identity changed; start a new run")); }
    crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,bindings,registry).with_execution_limits(request.execution_limits),cancellation,on_progress)
}

pub(super) fn verify_version(processor: &AudioToolPin, settings: &AllenkRepairConfiguration, directory: &Path, cancellation: &CancellationToken) -> Result<String> {
    let output = run_tool_isolated(processor,"allenk-image-repair",&["--version".into(),"--no-banner".into()],&settings.import.tool_limits(),cancellation,GroupPolicy::Inherit,Some(directory))
        .map_err(|diagnostic| Error::new(ErrorCategory::Dependency,format!("allenk version observation failed ({:?}): {}",diagnostic.code,diagnostic.message)))?;
    let version = std::str::from_utf8(&output.stdout).map_err(|_| invalid("allenk version output is not UTF-8"))?.trim();
    if version != ALLENK_INTERFACE_VERSION || version != processor.version || !output.stderr.is_empty() {
        return Err(Error::new(ErrorCategory::Dependency,"allenk version observation differs from the pinned basic-image interface"));
    }
    Ok(version.to_owned())
}

/// Decode named authority before normalizing into the shared image loop.
pub fn execute_invocation(request: &crate::ProviderInvocationRequest) -> Result<()> {
    request.validate()?;
    let operation: AllenkRepairOperation = serde_json::from_value(serde_json::to_value(&request.configuration.values).map_err(|error| invalid(error.to_string()))?)
        .map_err(|_| invalid("invalid closed allenk repair operation"))?;
    operation.validate()?;
    if operation.provider_configuration()? != request.configuration || request.stage_id != "repair_frames" { return Err(invalid("allenk invocation differs from its exact named operation")); }
    super::provider::execute_resolved(request,operation.normalized(),Some(&operation.settings))
}

/// Fresh dependency observations and exact predecessor authority also guard
/// generic v3 run/resume, including calls which would reuse every checkpoint.
pub(crate) fn preflight_plan(plan: &crate::PipelineV3Plan, registry: &crate::ProviderRegistry, cancellation: &CancellationToken) -> Result<()> {
    let named: Vec<_> = plan.payload.stages.iter().filter(|stage| stage.capability.id == ALLENK_REPAIR_CAPABILITY_ID).collect();
    if named.len() > 1 { return Err(invalid("allenk repair admits one fixed whole-frame-set stage")); }
    for stage in named {
        if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution,"allenk repair preflight cancelled")); }
        let registration = registry.registration(&stage.provider_lock.payload.registration_id).ok_or_else(|| invalid("allenk registration is missing"))?;
        let config = registration.configuration();
        let operation: AllenkRepairOperation = serde_json::from_value(serde_json::to_value(&config.values).map_err(|error| invalid(error.to_string()))?)
            .map_err(|_| invalid("invalid named allenk preflight operation"))?;
        if stage.capability.version != "1.0.0" || stage.provider_lock.payload.provider.id != ALLENK_REPAIR_PROVIDER_ID
            || operation.provider_configuration()? != *config
            || config.effective_configuration_sha256 != stage.provider_lock.payload.effective_configuration_sha256 {
            return Err(invalid("allenk operation differs from its locked named authority"));
        }
        let importer = plan.payload.stages.iter().find(|candidate| candidate.id == "import_frames").ok_or_else(|| invalid("allenk repair requires the same-run importer"))?;
        let importer_registration = registry.registration(&importer.provider_lock.payload.registration_id).ok_or_else(|| invalid("allenk import registration is missing"))?;
        let import_config = operation.settings.import.provider_configuration()?;
        let inputs = |stage: &crate::ResolvedPipelineStage, expected: &[(&str,&str)]| stage.inputs.len() == expected.len() && expected.iter().all(|(port,id)| stage.inputs.iter().any(|binding| binding.port == *port && binding.artifacts == [(*id).to_owned()]));
        let outputs = |stage: &crate::ResolvedPipelineStage, expected: &[(&str,&str)]| stage.outputs.len() == expected.len() && expected.iter().all(|(port,id)| stage.outputs.iter().any(|binding| binding.port == *port && binding.artifacts.len() == 1 && binding.artifacts[0].id == *id));
        if stage.id != "repair_frames" || stage.depends_on != ["import_frames"]
            || !inputs(stage,&[("frames","imported_frames"),("import","import_evidence"),("index","source_index")])
            || !outputs(stage,&[("frames","repaired_frames"),("index","repaired_index"),("evidence","repair_evidence")])
            || importer.capability.id != crate::frame_import::FRAME_IMPORT_CAPABILITY_ID || importer.capability.version != "1.0.0"
            || importer.provider_lock.payload.provider.id != crate::frame_import::FRAME_IMPORT_PROVIDER_ID || !importer.depends_on.is_empty()
            || importer_registration.configuration() != &import_config
            || importer.provider_lock.payload.effective_configuration_sha256 != import_config.effective_configuration_sha256
            || !inputs(importer,&[("source","source_video"),("frames","source_frames"),("index","source_index")])
            || !outputs(importer,&[("frames","imported_frames"),("evidence","import_evidence")])
            || ["source_video","source_frames","source_index"].iter().any(|id| !plan.payload.inputs.iter().any(|input| input.id == *id)) {
            return Err(invalid("allenk repair requires the exact same-run import producer and source bindings; loose import evidence cannot be adopted"));
        }
        for (pin,id) in [(&operation.settings.tool,"allenk-image-repair"),(&operation.settings.import.ffmpeg,"ffmpeg")] {
            if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution,"allenk dependency preflight cancelled")); }
            verify_pin(pin,id).map_err(|_| Error::new(ErrorCategory::Dependency,"allenk repair executable or decoder pin changed"))?;
            identity(&pin.executable,super::MAXIMUM_TOOL_BYTES)?;
        }
    }
    Ok(())
}
