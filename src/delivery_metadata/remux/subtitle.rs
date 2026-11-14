//! Closed binding to one current native soft-caption child, never a guessed
//! extra-track selection or a different source hidden behind a generic path.
use super::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubtitlePreservation {
    pub schema: String,
    pub run_directory: PathBuf,
    pub artifact_id: String,
    pub completion: crate::final_video::FinalVideoHandoff,
    pub document: crate::timed_text::TimedTextDocument,
    pub stream_index: u32,
}
impl SubtitlePreservation {
    pub fn observe(root: &Path, artifact_id: &str, token: &CancellationToken) -> Result<Self> {
        let completion = crate::final_video::observe_captions(root)?;
        let artifact = completion
            .artifacts
            .iter()
            .find(|a| a.artifact_id == artifact_id)
            .ok_or_else(|| invalid("selected native soft-caption artifact is missing"))?;
        let report = crate::subtitle_delivery::delivery_report(root, token)?;
        if artifact.variant != crate::final_video::FinalVideoVariant::SubtitleMuxedCandidate
            || report.operation.request.configuration.mode
                != crate::subtitle_delivery::CaptionMode::SoftMux
            || !report.media.authored_subtitle_track
            || !report.media.video_packets_preserved
            || artifact.sha256 != report.media.output_sha256
            || artifact.byte_count != report.media.output_byte_count
        {
            return Err(invalid(
                "subtitle preservation requires the exact current accepted native soft-caption source",
            ));
        }
        let stream_index = if artifact.audio.is_some() { 2 } else { 1 };
        let value = Self {
            schema: "aniflow.accepted-subtitle-preservation/v1".into(),
            run_directory: root.to_path_buf(),
            artifact_id: artifact_id.into(),
            completion,
            document: report.operation.lyrics.document,
            stream_index,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        self.document.validate()?;
        let artifact = self.artifact()?;
        if self.schema != "aniflow.accepted-subtitle-preservation/v1"
            || self.stream_index != (if artifact.audio.is_some() { 2 } else { 1 })
            || artifact.variant != crate::final_video::FinalVideoVariant::SubtitleMuxedCandidate
            || self.document.cues.is_empty()
            || self.document.cues.len() > 2048
        {
            return Err(invalid(
                "invalid bounded accepted-subtitle preservation binding",
            ));
        }
        Ok(())
    }
    pub(crate) fn artifact(&self) -> Result<&crate::final_video::FinalVideoArtifact> {
        self.completion
            .artifacts
            .iter()
            .find(|a| a.artifact_id == self.artifact_id)
            .ok_or_else(|| {
                invalid("subtitle preservation artifact is not a member of its completion")
            })
    }
    pub(crate) fn recheck(&self, source: &Path, token: &CancellationToken) -> Result<()> {
        self.validate()?;
        if self.artifact()?.locator != source
            || Self::observe(&self.run_directory, &self.artifact_id, token)? != *self
        {
            return Err(invalid(
                "accepted subtitle source changed before/after metadata remux",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreservedSubtitleEvidence {
    pub schema: String,
    pub source_stream: u32,
    pub output_stream: u32,
    pub packet_configuration_timing_sha256: String,
    pub text_timing_sha256: String,
    pub packet_count: u64,
    pub complete_text_roundtrip: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> SubtitlePreservation {
        let completion=serde_json::from_value(json!({"schema":"aniflow.final-video-handoff/v1","completion_id":"a".repeat(64),"plan_sha256":"b".repeat(64),"run_manifest_sha256":"c".repeat(64),"delivery_acceptance_sha256":"d".repeat(64),"source_lineage":[],"creative_approval":"not_performed","publication_authority":"none","proof_state":"not_asserted",
            "artifacts":[{"artifact_id":"caption_video","variant":"subtitle_muxed_candidate","locator":"/synthetic/captions.mp4","sha256":"e".repeat(64),"byte_count":1000,
                "width":1920,"height":1080,"frame_count":30,"frame_rate":{"numerator":30,"denominator":1},"video_interval":{"start":{"numerator":0,"denominator":1},"end":{"numerator":1,"denominator":1}},"video_duration":{"numerator":1,"denominator":1},
                "video_codec":"h264","pixel_format":"yuv420p","color_primaries":"bt709","color_transfer":"bt709","color_matrix":"bt709","color_range":"tv","sample_aspect_ratio":"1:1","orientation_degrees":0,"audio":null,"metadata":null,
                "producer_lock_sha256":"1".repeat(64),"stage_invocation_sha256":"2".repeat(64),"stage_checkpoint_sha256":"3".repeat(64),"private_report_sha256":"4".repeat(64)}]})).unwrap();
        let document = crate::timed_text::decode(
            b"1\n00:00:00,100 --> 00:00:00,900\nExact words\n",
            crate::timed_text::TimedTextFormat::Srt,
            &crate::timed_text::ImportContext::default(),
        )
        .unwrap()
        .document;
        SubtitlePreservation {
            schema: "aniflow.accepted-subtitle-preservation/v1".into(),
            run_directory: "/synthetic/native-caption".into(),
            artifact_id: "caption_video".into(),
            completion,
            document,
            stream_index: 1,
        }
    }
    fn operation() -> RemuxOperation {
        let binding = binding();
        let settings=AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&json!({"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},"delivery_metadata":{"schema":"aniflow.delivery-metadata-policy/v1","mode":"minimal","approved":{}}})).unwrap()).unwrap();
        RemuxOperation {
            schema: "aniflow.metadata-remux-operation/v2".into(),
            settings,
            settings_schema_sha256: crate::video_assembly::configuration_schema_reference().sha256,
            source_sha256: "e".repeat(64),
            source_byte_count: 1000,
            selection: StreamSelection {
                video_stream: Some(0),
                audio_stream: None,
                no_audio: true,
                discard_streams: vec![1],
                audio_extent: Some(crate::temporal::AudioExtentPolicy::PreserveExact),
            },
            subtitle_preservation: Some(binding),
        }
    }
    #[test]
    fn v2_requires_the_exact_native_subject_and_declared_subtitle_membership() {
        let op = operation();
        op.validate().unwrap();
        let mut changed = op.clone();
        changed.selection.discard_streams.clear();
        assert!(changed.validate().is_err());
        let mut changed = op.clone();
        changed.selection.discard_streams.push(9);
        assert!(changed.validate().is_err());
        let mut changed = op;
        changed.source_sha256 = "f".repeat(64);
        assert!(changed.validate().is_err());
    }
    #[test]
    fn v1_never_silently_acquires_subtitle_semantics_and_none_stays_omitted() {
        let mut op = operation();
        op.schema = SCHEMA.into();
        assert!(op.validate().is_err());
        op.subtitle_preservation = None;
        op.selection.discard_streams.clear();
        op.validate().unwrap();
        assert!(
            serde_json::to_value(&op)
                .unwrap()
                .get("subtitle_preservation")
                .is_none()
        );
        assert_eq!(op.schema().id, SCHEMA);
        assert_eq!(op.schema().version, "1.0.0");
    }
    #[test]
    fn text_and_millisecond_changes_alter_the_preserved_track_subject() {
        let original = binding().document;
        let before = crate::video_assembly::media::subtitle_text_identity(&original).unwrap();
        for bytes in [
            b"1\n00:00:00,100 --> 00:00:00,901\nExact words\n".as_slice(),
            b"1\n00:00:00,100 --> 00:00:00,900\nOther words\n".as_slice(),
        ] {
            let changed = crate::timed_text::decode(
                bytes,
                crate::timed_text::TimedTextFormat::Srt,
                &crate::timed_text::ImportContext::default(),
            )
            .unwrap()
            .document;
            assert_ne!(
                before,
                crate::video_assembly::media::subtitle_text_identity(&changed).unwrap()
            );
        }
    }
    #[test]
    fn serialized_shape_without_native_caption_acceptance_cannot_be_adopted() {
        let binding = binding();
        binding.validate().unwrap();
        assert!(
            binding
                .recheck(
                    &binding.artifact().unwrap().locator,
                    &CancellationToken::default()
                )
                .is_err()
        );
    }
}
