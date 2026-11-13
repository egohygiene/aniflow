//! Inert selected-role mapping; no source media, model, font or process port.
use super::*;
use crate::destination::{
    DestinationProfile,
    video::{GeometryRule, VideoExportConfiguration},
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestinationVideoSetupOptions {
    pub media: FinishingMediaOptions,
    pub geometry: GeometryRule,
    pub allow_upscale: bool,
    pub profile: DestinationProfile,
    pub evaluated_on: chrono::NaiveDate,
    pub metadata: crate::delivery_metadata::DeliveryMetadataPolicy,
}
impl DestinationVideoSetupOptions {
    pub(super) fn configuration(
        &self,
        ffmpeg: AudioToolPin,
        ffprobe: AudioToolPin,
    ) -> VideoExportConfiguration {
        let m = &self.media;
        VideoExportConfiguration {
            schema: "aniflow.destination-video-configuration/v1".into(),
            geometry: self.geometry.clone(),
            allow_upscale: self.allow_upscale,
            profile: self.profile.clone(),
            evaluated_on: self.evaluated_on,
            tools: crate::video_assembly::AssemblyConfiguration {
                schema: crate::video_assembly::ASSEMBLY_CONFIGURATION_SCHEMA_V1.into(),
                ffmpeg,
                ffprobe,
                timeout_milliseconds: m.timeout_milliseconds,
                maximum_capture_bytes: m.maximum_capture_bytes,
                maximum_source_bytes: m.maximum_source_bytes,
                maximum_output_bytes: m.maximum_output_bytes,
                maximum_frame_bytes: m.maximum_frame_bytes,
                maximum_total_frame_bytes: m.maximum_total_frame_bytes,
                maximum_pixels: m.maximum_pixels,
                maximum_working_bytes: m.maximum_working_bytes,
                minimum_free_bytes: m.minimum_free_bytes,
                delivery_crf: m.delivery_crf,
                preset: m.preset.clone(),
                delivery_metadata: Some(self.metadata.clone()),
            },
        }
    }
}
pub(super) const PROBE_FLAGS: &[&str] = &[
    "-version",
    "-v",
    "-protocol_whitelist",
    "-select_streams",
    "-show_streams",
    "-show_format",
    "-show_chapters",
    "-show_programs",
    "-show_frames",
    "-show_packets",
    "-show_data_hash",
    "-show_entries",
    "-of",
    "-i",
];
pub(super) const PROBE_FEATURES: &[&str] = &[
    "demuxer:mov",
    "protocol:file",
    "protocol:pipe",
    "decoder:h264",
    "decoder:ffv1",
    "decoder:aac",
];
pub(super) fn requirements(geometry: &GeometryRule) -> (Vec<&'static str>, Vec<&'static str>) {
    let flags = vec![
        "-version",
        "-hide_banner",
        "-nostdin",
        "-v",
        "-xerror",
        "-err_detect",
        "-threads",
        "-protocol_whitelist",
        "-noautorotate",
        "-copyts",
        "-copytb",
        "-filter_threads",
        "-i",
        "-map",
        "-sn",
        "-dn",
        "-vf",
        "-c:v",
        "-preset",
        "-crf",
        "-bf",
        "-threads:v",
        "-pix_fmt",
        "-color_primaries",
        "-color_trc",
        "-colorspace",
        "-color_range",
        "-fps_mode:v",
        "-enc_time_base:v",
        "-video_track_timescale",
        "-movie_timescale",
        "-c:a",
        "-copypriorss:a",
        "-map_metadata",
        "-map_chapters",
        "-metadata",
        "-movflags",
        "-avoid_negative_ts",
        "-use_editlist",
        "-fs",
        "-n",
        "-f",
    ];
    let mut features = vec![
        "protocol:file",
        "protocol:pipe",
        "demuxer:mov",
        "decoder:h264",
        "decoder:ffv1",
        "decoder:aac",
        "encoder:libx264",
        "encoder:wrapped_avframe",
        "muxer:mp4",
        "muxer:null",
        "filter:scale",
        "filter:setsar",
        "filter:format",
        "pixel_format:yuv420p",
        "hash:sha256",
    ];
    if matches!(geometry, GeometryRule::FitAndPad { .. }) {
        features.push("filter:pad");
    }
    (flags, features)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padded_geometry_requires_pad_while_exact_aspect_does_not() {
        let (_, exact) = requirements(&GeometryRule::ExactAspect {
            width: 1080,
            height: 1920,
        });
        let (_, padded) = requirements(&GeometryRule::FitAndPad {
            width: 1080,
            height: 1920,
        });
        assert!(!exact.contains(&"filter:pad"));
        assert!(padded.contains(&"filter:pad"));
        assert!(includes(&exact, &["filter:pad"], "ffmpeg", "features").is_err());
    }
    #[test]
    fn native_video_encoding_and_decode_requirements_are_separate_from_audio_encoding() {
        let (_, features) = requirements(&GeometryRule::FitWithin {
            maximum_width: 1080,
            maximum_height: 1920,
        });
        assert!(features.contains(&"encoder:libx264"));
        assert!(features.contains(&"decoder:ffv1"));
        assert!(features.contains(&"decoder:aac"));
        assert!(!features.contains(&"encoder:aac"));
    }
    #[test]
    fn preparation_selects_only_explicit_tool_roles() {
        let selection = MediaSetupSelection::DestinationVideo {
            ffmpeg_dependency_id: "chosen-encoder".into(),
            ffprobe_dependency_id: "chosen-probe".into(),
        };
        let deps = selection.dependencies();
        assert_eq!(deps.len(), 2);
        assert!(
            deps.iter()
                .all(|(_, kind)| *kind == ToolchainDependencyKind::Tool)
        );
        assert_eq!(deps[0].0, "chosen-encoder");
    }
    #[test]
    fn missing_codec_or_probe_observations_are_actionable_refusals() {
        assert!(
            includes(
                &["decoder:h264".into()],
                &["encoder:libx264"],
                "chosen-encoder",
                "features"
            )
            .is_err()
        );
        assert!(
            includes(
                &["-show_streams".into()],
                &["-show_packets"],
                "chosen-probe",
                "flags"
            )
            .is_err()
        );
    }
    #[test]
    fn export_readiness_needs_no_font_neural_model_or_automatic_install_feature() {
        let (flags, features) = requirements(&GeometryRule::ExactAspect {
            width: 1080,
            height: 1920,
        });
        for name in ["drawtext", "font", "vulkan", "model", "download", "install"] {
            assert!(!flags.iter().chain(&features).any(|v| v.contains(name)));
        }
    }
}
