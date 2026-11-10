use super::*;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPublishingQc {
    pub configuration:crate::audio_inspection::AudioInspectionConfiguration,pub maximum_pcm_bytes:u64,pub maximum_working_bytes:u64,pub minimum_free_bytes:u64,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPublishingRequest {
    pub schema:String,pub subject:FinalLosslessSubject,pub provider_executable:PathBuf,
    pub export:Option<LosslessConfiguration>,pub audio_qc:Option<AudioPublishingQc>,
    pub profile:destination::DestinationProfile,pub evaluated_on:chrono::NaiveDate,pub package:PublishingPackage,pub retention:Option<PublishingRetention>,
}
impl AudioPublishingRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>destination::MAXIMUM_REQUEST_BYTES{return Err(state("audio publishing request exceeds 64 KiB"));}
        let value:Self=crate::provider::decode_json(bytes,"audio publishing request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        self.profile.validate()?;self.package.public_metadata.validate()?;
        if !matches!(self.schema.as_str(),"aniflow.audio-publishing-request/v1"|"aniflow.audio-publishing-request/v2")||!matches!(self.profile.destination,destination::Destination::Soundcloud|destination::Destination::Distrokid)
            ||!matches!(self.profile.requirements,destination::MediaRequirements::Pcm {..})||self.profile.require_loudness_evidence&&self.audio_qc.is_none()
            ||!(1..=1_099_511_627_776).contains(&self.package.maximum_package_bytes)||self.package.minimum_free_bytes>1_099_511_627_776 {
            return Err(state("audio publishing requires one supplied dated manual SoundCloud/DistroKid PCM profile, explicit public metadata/bounds and final QC when loudness is required"));
        }
        if let Some(export)=&self.export {export.validate()?;if !matches!(self.subject,FinalLosslessSubject::AcceptedPcm {..}){return Err(state("native audio export requires original accepted PCM; existing exports can be selected directly"));}}
        if self.audio_qc.is_some(){qc_request(self,self.subject.clone())?.validate()?;}
        if self.retention.as_ref().is_some_and(|r|r.maximum_bundle_bytes==0||r.maximum_bundle_bytes>1_099_511_627_776||r.minimum_free_bytes>1_099_511_627_776
            ||!self.package.public_metadata.release_id.as_bytes()[0].is_ascii_alphanumeric()||!self.package.public_metadata.version.as_bytes()[0].is_ascii_alphanumeric()) {
            return Err(state("audio publishing retention handoff requires finite existing-owner bounds and normalized release/version tokens"));
        }Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPublishingIntent {
    pub schema:String,pub request:AudioPublishingRequest,pub source:destination::MediaObservation,pub components:Vec<PublishingComponent>,pub export_plan:Option<PipelineV3Plan>,pub intent_sha256:String,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPublishingPlan {pub schema:String,pub intent:AudioPublishingIntent,pub steps:Vec<PublishingStep>}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPublishingStatus {
    pub schema:String,pub run_directory:PathBuf,pub intent_sha256:String,pub steps:Vec<PublishingStep>,
    pub final_media:Option<MediaInput>,pub qc_receipt_sha256:Option<String>,pub preflight:Option<PreflightReport>,
    pub posting_plan:Option<posting::PostingPlan>,pub review:Option<ManualReview>,pub package:Option<posting::PostingReceipt>,pub retention:Option<RetentionHandoff>,
    pub complete:bool,pub next_action:PublishingNextAction,pub creative_approval:String,pub publication_authority:String,
}
