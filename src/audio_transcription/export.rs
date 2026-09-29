//! Source-bound export through the registered loss-aware timed-text codecs.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::AudioTranscriptionReport;
use crate::audio_analysis::AudioArtifactReference;
use crate::timed_text::{
    ConversionFailure, ConversionOptions, FileConversionOutcome, TimedTextFormat,
    convert_embedded_file,
};

/// A completed export and the original report bytes from which it was derived.
///
/// `transcription_report` identifies the supplied report file. The nested
/// conversion report's `input` instead identifies the extracted timed-text
/// document's canonical JSON. The original report is rechecked before the
/// conversion completion marker is published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptExportOutcome {
    pub transcription_report: AudioArtifactReference,
    pub conversion: FileConversionOutcome,
}

/// Export an observed transcript without rerunning transcription or promoting
/// its authority. Empty and unavailable observations cannot invent text cues.
/// The output directory must be new, and every codec loss requires permission.
pub fn export_transcript_file(
    input: &Path,
    to: TimedTextFormat,
    output_directory: &Path,
    options: &ConversionOptions,
) -> Result<TranscriptExportOutcome, ConversionFailure> {
    let (mut transcription_report, conversion) =
        convert_embedded_file(input, to, output_directory, options, |bytes| {
            let report = AudioTranscriptionReport::from_json_slice(bytes)?;
            report.timed_text.ok_or_else(|| {
                crate::Error::new(
                    crate::ErrorCategory::Configuration,
                    "transcription report has no observed text to export",
                )
            })
        })?;
    transcription_report.id = "transcription_report".into();
    Ok(TranscriptExportOutcome {
        transcription_report,
        conversion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_report_never_reserves_an_export_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temporary.path()).unwrap();
        let source = root.join("report.json");
        let destination = root.join("export");
        std::fs::write(&source, b"{\"schema\":\"untrusted\",\"timed_text\":null}").unwrap();
        let before = std::fs::read(&source).unwrap();
        assert!(
            export_transcript_file(
                &source,
                TimedTextFormat::Srt,
                &destination,
                &ConversionOptions::default(),
            )
            .is_err()
        );
        assert!(!destination.exists());
        assert_eq!(std::fs::read(&source).unwrap(), before);
    }
}
