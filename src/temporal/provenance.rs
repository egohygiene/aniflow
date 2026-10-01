use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{
    TEMPORAL_ARTIFACT_INDEX_SCHEMA_V1, TemporalCode, TemporalInspection, TimeRange, diagnostic,
    sha256_file,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceStreamWindow {
    pub stream_index: u32,
    pub source_time: TimeRange,
    pub frame_ordinal: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactTimeBinding {
    pub artifact_path: String,
    pub artifact_sha256: String,
    pub source_sha256: String,
    pub temporal_inspection_sha256: String,
    pub source_streams: Vec<SourceStreamWindow>,
    /// Mapping is provenance, not a claim that transformed content is authentic.
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalArtifactIndex {
    pub schema: String,
    pub source_sha256: String,
    pub temporal_inspection_sha256: String,
    pub artifacts: Vec<ArtifactTimeBinding>,
}

impl TemporalArtifactIndex {
    pub fn new(source: &TemporalInspection) -> Result<Self> {
        source.require_processing()?;
        Ok(Self {
            schema: TEMPORAL_ARTIFACT_INDEX_SCHEMA_V1.to_owned(),
            source_sha256: source.source_sha256.clone(),
            temporal_inspection_sha256: source.sha256()?,
            artifacts: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactStreamRelation {
    Frame(u64),
    Video,
    Audio,
    AudioVideo,
}

/// Bind a workspace-confined regular file to exact selected source intervals.
/// Each relation selects only the source streams present in that artifact kind.
pub fn bind_artifact(
    root: &Path,
    artifact: &Path,
    source: &TemporalInspection,
    relation: ArtifactStreamRelation,
) -> Result<ArtifactTimeBinding> {
    source.require_processing()?;
    let canonical_root = root.canonicalize()?;
    let canonical = artifact.canonicalize()?;
    let relative = canonical.strip_prefix(&canonical_root).map_err(|_| {
        diagnostic(
            TemporalCode::TimelineMismatch,
            None,
            "temporal artifact must be inside its run workspace",
        )
    })?;
    if std::fs::symlink_metadata(artifact)?
        .file_type()
        .is_symlink()
        || !canonical.is_file()
    {
        return Err(diagnostic(
            TemporalCode::TimelineMismatch,
            None,
            "temporal artifact must be a regular nonsymlink file",
        )
        .into());
    }
    let selection = source.selected.as_ref().ok_or_else(|| {
        diagnostic(
            TemporalCode::InvalidSelection,
            None,
            "source selection is missing",
        )
    })?;
    let mut windows = Vec::new();
    if let ArtifactStreamRelation::Frame(ordinal) = relation {
        let timeline = source.video_timeline()?;
        let observed = usize::try_from(ordinal)
            .ok()
            .and_then(|i| timeline.frames.get(i))
            .ok_or_else(|| {
                diagnostic(
                    TemporalCode::FrameCountMismatch,
                    None,
                    "derived frame ordinal is outside the decoded source",
                )
            })?;
        windows.push(SourceStreamWindow {
            stream_index: observed.stream_index,
            source_time: observed.source_time,
            frame_ordinal: Some(ordinal),
        });
    } else {
        let video = selection.video.filter(|_| {
            matches!(
                relation,
                ArtifactStreamRelation::Video | ArtifactStreamRelation::AudioVideo
            )
        });
        let audio = selection.audio.filter(|_| {
            matches!(
                relation,
                ArtifactStreamRelation::Audio | ArtifactStreamRelation::AudioVideo
            )
        });
        for index in video.into_iter().chain(audio) {
            let range = source
                .timeline(index)
                .and_then(|t| t.source_time)
                .ok_or_else(|| {
                    diagnostic(
                        TemporalCode::MissingTemporalEvidence,
                        Some(index),
                        "source interval is unavailable",
                    )
                })?;
            windows.push(SourceStreamWindow {
                stream_index: index,
                source_time: range,
                frame_ordinal: None,
            });
        }
    }
    if windows.is_empty() {
        return Err(diagnostic(
            TemporalCode::MissingTemporalEvidence,
            None,
            "artifact has no source stream interval",
        )
        .into());
    }
    Ok(ArtifactTimeBinding {
        artifact_path: relative.to_string_lossy().replace('\\', "/"),
        artifact_sha256: sha256_file(&canonical)?,
        source_sha256: source.source_sha256.clone(),
        temporal_inspection_sha256: source.sha256()?,
        source_streams: windows,
        relation: if matches!(relation, ArtifactStreamRelation::Frame(_)) {
            "derived_frame_at_source_interval"
        } else {
            "derived_selected_streams_at_source_intervals"
        }
        .to_owned(),
    })
}
