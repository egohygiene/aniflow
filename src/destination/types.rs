use std::path::PathBuf;
use chrono::NaiveDate;
use serde::{Deserialize,Serialize};
use crate::temporal::RationalTime;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Destination {Tiktok,Soundcloud,Distrokid}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum DeliveryRoute {Manual}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum PcmEncoding {Pcm16,Pcm24,Float32}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum MediaInput {
    AcceptedGainCandidate {run_directory:PathBuf},
    AcceptedVideo {run_directory:PathBuf,artifact_id:String},
    AcceptedVideoWithQc {run_directory:PathBuf,artifact_id:String,qc_directory:PathBuf,qc_receipt_sha256:String},
    AcceptedPcm {file:PathBuf,signal_run_directory:PathBuf},
    AcceptedAudioExport {run_directory:PathBuf},
    AcceptedAudioWithQc {subject:crate::delivery_audio::final_lossless_qc::FinalLosslessSubject,qc_directory:PathBuf,qc_receipt_sha256:String},
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaFile {pub locator:PathBuf,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum MediaFacts {
    GainCandidate {completion:Box<crate::delivery_audio::gain::Completion>},
    Video {completion:crate::final_video::FinalVideoHandoff,artifact_id:String},
    VideoWithQc {completion:crate::final_video::FinalVideoHandoff,artifact_id:String,qc:Box<crate::delivery_audio::final_qc::FinalAudioQcReport>},
    Pcm {summary:crate::delivery_audio::DeliveryLoudnessSummary,encoding:PcmEncoding,pcm_sha256:String,run_manifest_sha256:String},
    LosslessAudio {report:Box<crate::lossless_audio::LosslessReport>},
    AudioWithQc {qc:Box<crate::delivery_audio::final_lossless_qc::FinalLosslessQcReport>},
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaObservation {pub input:MediaInput,pub file:MediaFile,pub facts:MediaFacts}

/// Requirement evidence is an operator-supplied declaration, not a live lookup.
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementEvidence {
    pub source_urls:Vec<String>,pub checked_on:NaiveDate,pub valid_through:NaiveDate,pub checked_by:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum MediaRequirements {
    /// First lane: accepted fixed H.264/yuv420p SDR MP4, never arbitrary files.
    Video {
        minimum_axis:u32,maximum_axis:u32,
        required_width:Option<u32>,required_height:Option<u32>,
        minimum_frame_rate:RationalTime,maximum_frame_rate:RationalTime,
        audio_required:bool,maximum_audio_channels:u64,
        require_zero_origin:bool,require_matching_audio_extent:bool,
    },
    /// First lossless lane is current accepted PCM WAV signal evidence.
    Pcm {sample_rates_hz:Vec<u32>,channels:Vec<u16>,encodings:Vec<PcmEncoding>},
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestinationProfile {
    pub schema:String,pub id:String,pub revision:String,pub destination:Destination,pub route:DeliveryRoute,
    pub evidence:RequirementEvidence,pub maximum_file_bytes:u64,pub maximum_duration:RationalTime,
    pub require_loudness_evidence:bool,pub requirements:MediaRequirements,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightRequest {
    pub schema:String,pub input:MediaInput,pub profile:DestinationProfile,
    /// Explicit evaluation date; replaying an old request does not refresh policy.
    pub evaluated_on:NaiveDate,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum CheckState {Matches,NeedsChanges,MissingEvidence}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementCheck {pub id:String,pub state:CheckState,pub reason:String}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum PreflightDisposition {MatchesSuppliedProfile,NeedsChanges,MissingEvidence}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightPayload {
    pub request:PreflightRequest,pub profile_sha256:String,pub media:MediaObservation,pub checks:Vec<RequirementCheck>,
    pub disposition:PreflightDisposition,pub requirement_authority:String,pub creative_approval:String,pub publication_authority:String,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightReport {pub schema:String,pub preflight_sha256:String,pub payload:PreflightPayload}
