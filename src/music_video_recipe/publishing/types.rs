use super::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingAudioQc {
    pub configuration: crate::video_assembly::AssemblyConfiguration,
    pub maximum_pcm_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingPackage {
    pub public_metadata: posting::PublicReleaseMetadata,
    pub metadata_scope_policy: posting::MetadataScopePolicy,
    pub maximum_package_bytes: u64,
    pub minimum_free_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingRetention {
    pub maximum_bundle_bytes: u64,
    pub minimum_free_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingRequest {
    pub schema: String,
    pub video_run_directory: PathBuf,
    pub artifact_id: String,
    pub provider_executable: PathBuf,
    pub export: Option<video::VideoExportConfiguration>,
    pub audio_qc: Option<PublishingAudioQc>,
    pub profile: destination::DestinationProfile,
    pub evaluated_on: chrono::NaiveDate,
    pub package: PublishingPackage,
    pub retention: Option<PublishingRetention>,
}
impl PublishingRequest {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > destination::MAXIMUM_REQUEST_BYTES {
            return Err(state("publishing request exceeds 64 KiB"));
        }
        let value: Self = crate::provider::decode_json(bytes, "fixed publishing request")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        self.profile.validate()?;
        self.package.public_metadata.validate()?;
        if !matches!(
            self.schema.as_str(),
            "aniflow.publishing-request/v1" | "aniflow.publishing-request/v2"
        ) || !destination::identifier(&self.artifact_id)
            || !matches!(
                self.profile.requirements,
                destination::MediaRequirements::Video { .. }
            )
            || self.profile.require_loudness_evidence && self.audio_qc.is_none()
            || !(1..=1_099_511_627_776).contains(&self.package.maximum_package_bytes)
            || self.package.minimum_free_bytes > 1_099_511_627_776
        {
            return Err(state(
                "fixed publishing requires one manual video profile, explicit package metadata/bounds and selected QC when loudness evidence is required",
            ));
        }
        if let Some(export) = &self.export {
            export.validate()?;
            if export.profile != self.profile || export.evaluated_on != self.evaluated_on {
                return Err(state(
                    "publishing export and preflight must use the same frozen profile and evaluation date",
                ));
            }
        }
        if let Some(qc) = &self.audio_qc {
            qc.configuration.validate()?;
            if !(1024..=crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES)
                .contains(&qc.maximum_pcm_bytes)
            {
                return Err(state("publishing QC requires a finite complete PCM bound"));
            }
        }
        if self.retention.as_ref().is_some_and(|r| {
            r.maximum_bundle_bytes == 0
                || r.maximum_bundle_bytes > 1_099_511_627_776
                || r.minimum_free_bytes > 1_099_511_627_776
                || !self.package.public_metadata.release_id.as_bytes()[0].is_ascii_alphanumeric()
                || !self.package.public_metadata.version.as_bytes()[0].is_ascii_alphanumeric()
        }) {
            return Err(state(
                "publishing retention handoff requires finite existing-owner bounds",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingComponent {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingIntent {
    pub schema: String,
    pub request: PublishingRequest,
    pub source: FinalVideoHandoff,
    pub components: Vec<PublishingComponent>,
    pub export_plan: Option<PipelineV3Plan>,
    pub intent_sha256: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishingPhase {
    DestinationExport,
    FinalAudioQc,
    DestinationPreflight,
    FinalReview,
    ManualPackage,
    RetentionHandoff,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishingStepState {
    Pending,
    Incomplete,
    AwaitingReview,
    Rejected,
    Blocked,
    Complete,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingStep {
    pub phase: PublishingPhase,
    pub state: PublishingStepState,
    pub run_directory: Option<PathBuf>,
    pub native_plan_sha256: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingPlan {
    pub schema: String,
    pub intent: PublishingIntent,
    pub steps: Vec<PublishingStep>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionHandoff {
    pub schema: String,
    pub request: crate::release_evidence::EvidenceRequest,
    pub plan: crate::release_evidence::EvidencePlan,
    pub sealing_state: String,
    pub publication_authority: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingNextAction {
    pub schema: String,
    pub phase: Option<PublishingPhase>,
    pub action: String,
    pub reason: String,
    pub review_context_sha256: Option<String>,
    pub review_subject_sha256: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingStatus {
    pub schema: String,
    pub run_directory: PathBuf,
    pub intent_sha256: String,
    pub steps: Vec<PublishingStep>,
    pub final_media: Option<MediaInput>,
    pub qc_receipt_sha256: Option<String>,
    pub preflight: Option<PreflightReport>,
    pub posting_plan: Option<posting::PostingPlan>,
    pub review: Option<ManualReview>,
    pub package: Option<posting::PostingReceipt>,
    pub retention: Option<RetentionHandoff>,
    pub complete: bool,
    pub next_action: PublishingNextAction,
    pub creative_approval: String,
    pub publication_authority: String,
}
