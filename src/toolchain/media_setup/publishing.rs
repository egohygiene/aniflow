//! Read-only publishing prerequisite projection over the existing setup owner.
use super::*;
use crate::audio_inspection::{
    AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1, AudioInspectionConfiguration,
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalVideoAudioQcSetupOptions {
    pub media: FinishingMediaOptions,
    pub maximum_pcm_bytes: u64,
}
impl FinalVideoAudioQcSetupOptions {
    pub(super) fn configuration(
        &self,
        ffmpeg: AudioToolPin,
        ffprobe: AudioToolPin,
    ) -> Result<Value> {
        if !(1024..=crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES)
            .contains(&self.maximum_pcm_bytes)
        {
            return Err(invalid("QC requires a finite complete PCM byte bound"));
        }
        let configuration = self.media.configuration(ffmpeg, ffprobe, None);
        configuration.validate()?;
        if configuration.ffmpeg.version != "8.0.1" || configuration.ffprobe.version != "8.0.1" {
            return Err(invalid(
                "native audio QC requires the existing exact 8.0.1 meter profile",
            ));
        }
        json_value(
            &serde_json::json!({"schema":"aniflow.final-video-audio-qc-setup/v1","configuration":configuration,"maximum_pcm_bytes":self.maximum_pcm_bytes}),
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataSubtitleSetup {
    None,
    PreserveAcceptedSoftSubtitle,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataRemuxSetupOptions {
    pub media: FinishingMediaOptions,
    pub policy: crate::delivery_metadata::DeliveryMetadataPolicy,
    pub subtitles: MetadataSubtitleSetup,
}
impl MetadataRemuxSetupOptions {
    pub(super) fn configuration(
        &self,
        ffmpeg: AudioToolPin,
        ffprobe: AudioToolPin,
    ) -> Result<Value> {
        let configuration = self
            .media
            .configuration(ffmpeg, ffprobe, Some(self.policy.clone()));
        configuration.validate()?;
        json_value(&configuration)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LosslessAudioSetupOptions {
    pub format: crate::lossless_audio::LosslessFormat,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
    pub maximum_output_bytes: u64,
    pub maximum_working_bytes: u64,
    pub minimum_free_bytes: u64,
    pub metadata_policy: crate::lossless_audio::ExportMetadataPolicy,
}
impl LosslessAudioSetupOptions {
    pub(super) fn configuration(
        &self,
        ffmpeg: AudioToolPin,
        ffprobe: AudioToolPin,
    ) -> crate::lossless_audio::LosslessConfiguration {
        crate::lossless_audio::LosslessConfiguration {
            schema: "aniflow.lossless-audio-configuration/v1".into(),
            format: self.format,
            tools: AudioInspectionConfiguration {
                schema: AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.into(),
                ffmpeg,
                ffprobe,
                tool_timeout_milliseconds: self.tool_timeout_milliseconds,
                maximum_tool_output_bytes: self.maximum_tool_output_bytes,
            },
            metadata_policy: self.metadata_policy,
            maximum_output_bytes: self.maximum_output_bytes,
            maximum_working_bytes: self.maximum_working_bytes,
            minimum_free_bytes: self.minimum_free_bytes,
        }
    }
}
fn json_value<T: Serialize>(value: &T) -> Result<Value> {
    serde_json::to_value(value).map_err(|_| invalid("cannot encode publishing setup configuration"))
}
impl FinishingMediaOptions {
    pub(super) fn configuration(
        &self,
        ffmpeg: AudioToolPin,
        ffprobe: AudioToolPin,
        delivery_metadata: Option<crate::delivery_metadata::DeliveryMetadataPolicy>,
    ) -> crate::video_assembly::AssemblyConfiguration {
        crate::video_assembly::AssemblyConfiguration {
            schema: crate::video_assembly::ASSEMBLY_CONFIGURATION_SCHEMA_V1.into(),
            ffmpeg,
            ffprobe,
            timeout_milliseconds: self.timeout_milliseconds,
            maximum_capture_bytes: self.maximum_capture_bytes,
            maximum_source_bytes: self.maximum_source_bytes,
            maximum_output_bytes: self.maximum_output_bytes,
            maximum_frame_bytes: self.maximum_frame_bytes,
            maximum_total_frame_bytes: self.maximum_total_frame_bytes,
            maximum_pixels: self.maximum_pixels,
            maximum_working_bytes: self.maximum_working_bytes,
            minimum_free_bytes: self.minimum_free_bytes,
            delivery_crf: self.delivery_crf,
            preset: self.preset.clone(),
            delivery_metadata,
        }
    }
}
pub(super) fn qc_requirements() -> (Vec<&'static str>, Vec<&'static str>) {
    (
        vec![
            "-version",
            "-hide_banner",
            "-nostdin",
            "-nostats",
            "-v",
            "-loglevel",
            "-xerror",
            "-err_detect",
            "-threads",
            "-protocol_whitelist",
            "-i",
            "-map",
            "-vn",
            "-sn",
            "-dn",
            "-map_metadata",
            "-map_chapters",
            "-c:a",
            "-flags:a",
            "-fflags",
            "-threads:a",
            "-rf64",
            "-fs",
            "-f",
            "-n",
            "-af",
        ],
        vec![
            "protocol:file",
            "protocol:pipe",
            "demuxer:mov",
            "decoder:h264",
            "decoder:ffv1",
            "decoder:aac",
            "demuxer:wav",
            "encoder:pcm_f32le",
            "decoder:pcm_f32le",
            "muxer:wav",
            "muxer:null",
            "filter:ebur128",
            "filter:ametadata",
            "filter:apad",
        ],
    )
}
pub(super) fn metadata_requirements(
    subtitles: MetadataSubtitleSetup,
) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut flags = vec![
        "-version",
        "-hide_banner",
        "-nostdin",
        "-v",
        "-xerror",
        "-err_detect",
        "-threads",
        "-protocol_whitelist",
        "-copyts",
        "-copytb",
        "-noautorotate",
        "-i",
        "-map",
        "-sn",
        "-dn",
        "-c",
        "-map_metadata",
        "-map_metadata:s:v:0",
        "-map_metadata:s:a:0",
        "-metadata:s:v:0",
        "-metadata:s:a:0",
        "-map_chapters",
        "-metadata",
        "-movflags",
        "-avoid_negative_ts",
        "-use_editlist",
        "-f",
        "-n",
    ];
    let mut features = vec![
        "protocol:file",
        "protocol:pipe",
        "demuxer:mov",
        "decoder:h264",
        "decoder:aac",
        "encoder:wrapped_avframe",
        "encoder:pcm_s16le",
        "muxer:mp4",
        "muxer:null",
    ];
    if subtitles == MetadataSubtitleSetup::PreserveAcceptedSoftSubtitle {
        flags.extend(["-disposition:s:0", "-map_metadata:s:s:0", "-metadata:s:s:0"]);
        features.extend(["decoder:mov_text", "encoder:srt", "muxer:srt"]);
    }
    (flags, features)
}
pub(super) fn lossless_requirements(
    format: crate::lossless_audio::LosslessFormat,
) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut flags = vec![
        "-version",
        "-hide_banner",
        "-nostdin",
        "-v",
        "-xerror",
        "-err_detect",
        "-threads",
        "-protocol_whitelist",
        "-i",
        "-map",
        "-vn",
        "-sn",
        "-dn",
        "-codec:a",
        "-f",
        "-hash",
    ];
    let mut features = vec![
        "protocol:file",
        "protocol:pipe",
        "demuxer:wav",
        "decoder:pcm_s16le",
        "decoder:pcm_s24le",
        "encoder:pcm_s16le",
        "encoder:pcm_s24le",
        "muxer:hash",
        "hash:sha256",
    ];
    if format == crate::lossless_audio::LosslessFormat::Flac {
        flags.extend([
            "-sample_fmt",
            "-bits_per_raw_sample",
            "-compression_level",
            "-map_metadata",
            "-map_chapters",
            "-fflags",
            "-flags:a",
            "-n",
        ]);
        features.extend(["demuxer:flac", "decoder:flac", "encoder:flac"]);
    } else {
        features.extend(["decoder:pcm_f32le", "encoder:pcm_f32le"]);
    }
    (flags, features)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingSetupChoice {
    pub capability_id: String,
    pub selection: MediaSetupSelection,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingSetupRequest {
    pub schema: String,
    pub profile: ToolchainProfile,
    pub inventory: ToolchainInventory,
    pub selections: Vec<PublishingSetupChoice>,
}
impl PublishingSetupRequest {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_json_slice(&super::super::types::read_json(path.as_ref())?)
    }
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAXIMUM_REQUEST_BYTES {
            return Err(invalid("publishing setup request exceeds one MiB"));
        }
        let value: Self = super::super::types::decode_unique_json(bytes)?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != "aniflow.toolchain.publishing-setup/v1"
            || !(1..=8).contains(&self.selections.len())
        {
            return Err(invalid(
                "publishing preparation requires 1..8 explicit capability selections",
            ));
        }
        let mut ids = BTreeSet::new();
        for choice in &self.selections {
            if !ids.insert(&choice.capability_id)
                || !matches!(
                    choice.selection,
                    MediaSetupSelection::FinalLosslessAudioQc { .. }
                        | MediaSetupSelection::FinalVideoAudioQc { .. }
                        | MediaSetupSelection::MetadataRemux { .. }
                        | MediaSetupSelection::LosslessAudioExport { .. }
                        | MediaSetupSelection::DestinationVideo { .. }
                        | MediaSetupSelection::CaptionRendering { .. }
                )
            {
                return Err(invalid(
                    "publishing selections must be distinct existing publishing capabilities",
                ));
            }
            self.selected(choice).validate()?;
        }
        Ok(())
    }
    fn selected(&self, choice: &PublishingSetupChoice) -> MediaSetupRequest {
        MediaSetupRequest {
            schema: MEDIA_SETUP_REQUEST_SCHEMA.into(),
            profile: self.profile.clone(),
            inventory: self.inventory.clone(),
            capability_id: choice.capability_id.clone(),
            selection: choice.selection.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingSetupStep {
    pub capability_id: String,
    pub selection: MediaSetupSelection,
    pub preparation: MediaSetupPreparation,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishingSetupPreparation {
    pub schema: String,
    pub request_sha256: String,
    pub ready: bool,
    pub selections: Vec<PublishingSetupStep>,
    pub diagnostics: Vec<String>,
    pub native_qualification: bool,
    pub host_runtime_evidence: String,
    pub source_media_evidence: String,
    pub provider_evidence: String,
    pub publication_authority: String,
}
/// All explicitly selected dependencies must agree. No process or media read.
pub fn prepare_publishing_setup(
    request: &PublishingSetupRequest,
) -> Result<PublishingSetupPreparation> {
    request.validate()?;
    if crate::provider::canonical_json_bytes(request)?.len() > MAXIMUM_REQUEST_BYTES {
        return Err(invalid("publishing setup request exceeds one MiB"));
    }
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    for choice in &request.selections {
        let preparation = prepare_media_setup(&request.selected(choice))?;
        diagnostics.extend(
            preparation
                .diagnostics
                .iter()
                .map(|d| format!("{}: {d}", choice.capability_id)),
        );
        selections.push(PublishingSetupStep {
            capability_id: choice.capability_id.clone(),
            selection: choice.selection.clone(),
            preparation,
        });
    }
    let value = PublishingSetupPreparation {
        schema: "aniflow.toolchain.publishing-setup-preparation/v1".into(),
        request_sha256: crate::provider::canonical_sha256(request)?,
        ready: selections.iter().all(|s| s.preparation.ready),
        selections,
        diagnostics,
        native_qualification: false,
        host_runtime_evidence: "unobserved".into(),
        source_media_evidence: "unobserved".into(),
        provider_evidence:
            "unobserved; native facade provider is supplied and checked at planning/execution"
                .into(),
        publication_authority: "none".into(),
    };
    if crate::provider::canonical_json_bytes(&value)?.len() > MAXIMUM_PREPARATION_BYTES {
        return Err(invalid("publishing preparation exceeds eight MiB"));
    }
    Ok(value)
}
#[cfg(test)]
mod tests;

/// Source-free settings for the exact final WAV/FLAC QC owner. Subject and
/// provider are supplied only to the actual QC request, never inferred here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalLosslessQcSetupOptions {
    pub format: crate::lossless_audio::LosslessFormat,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
    pub maximum_pcm_bytes: u64,
    pub maximum_working_bytes: u64,
    pub minimum_free_bytes: u64,
}
impl FinalLosslessQcSetupOptions {
    pub(super) fn configuration(
        &self,
        ffmpeg: AudioToolPin,
        ffprobe: AudioToolPin,
    ) -> Result<Value> {
        let configuration = AudioInspectionConfiguration {
            schema: AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.into(),
            ffmpeg,
            ffprobe,
            tool_timeout_milliseconds: self.tool_timeout_milliseconds,
            maximum_tool_output_bytes: self.maximum_tool_output_bytes,
        };
        let request=crate::delivery_audio::final_lossless_qc::FinalLosslessQcRequest {schema:"aniflow.final-lossless-audio-qc-request/v1".into(),subject:crate::delivery_audio::final_lossless_qc::FinalLosslessSubject::AcceptedAudioExport {run_directory:"/declared/accepted-native-export".into()},configuration:configuration.clone(),provider_executable:"/declared/provider".into(),maximum_pcm_bytes:self.maximum_pcm_bytes,maximum_working_bytes:self.maximum_working_bytes,minimum_free_bytes:self.minimum_free_bytes};
        request.validate()?;
        json_value(
            &serde_json::json!({"schema":"aniflow.final-lossless-audio-qc-setup/v1","configuration":configuration,"maximum_pcm_bytes":self.maximum_pcm_bytes,"maximum_working_bytes":self.maximum_working_bytes,"minimum_free_bytes":self.minimum_free_bytes}),
        )
    }
}
pub(super) fn final_lossless_qc_requirements(
    format: crate::lossless_audio::LosslessFormat,
) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut flags = vec![
        "-version",
        "-hide_banner",
        "-nostdin",
        "-nostats",
        "-v",
        "-loglevel",
        "-xerror",
        "-err_detect",
        "-threads",
        "-protocol_whitelist",
        "-i",
        "-map",
        "-vn",
        "-sn",
        "-dn",
        "-codec:a",
        "-f",
        "-hash",
        "-af",
    ];
    let mut features = vec![
        "protocol:file",
        "protocol:pipe",
        "demuxer:wav",
        "decoder:pcm_s16le",
        "decoder:pcm_s24le",
        "decoder:pcm_f32le",
        "encoder:pcm_s16le",
        "encoder:pcm_s24le",
        "encoder:pcm_f32le",
        "muxer:hash",
        "hash:sha256",
        "muxer:null",
        "filter:ebur128",
        "filter:ametadata",
        "filter:apad",
    ];
    if format == crate::lossless_audio::LosslessFormat::Flac {
        flags.extend([
            "-map_metadata",
            "-map_chapters",
            "-flags:a",
            "-fflags",
            "-threads:a",
            "-fs",
            "-n",
        ]);
        features.extend(["demuxer:flac", "decoder:flac", "muxer:s16le", "muxer:s24le"]);
    }
    (flags, features)
}
