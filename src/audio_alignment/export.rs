//! Export proposed alignment timing through the existing loss-aware codecs.
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::AudioAlignmentReport;
use crate::audio_analysis::AudioArtifactReference;
use crate::timed_text::{
    ConversionFailure, ConversionOptions, FileConversionOutcome, TimedTextFormat,
    convert_embedded_file,
};

/// Alignment never grants human review authority to generated timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentTimingAuthority {
    Candidate,
}

/// The original report is rechecked before publication. The conversion input
/// identifies extracted canonical timed-text JSON, not the enclosing report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentExportOutcome {
    pub alignment_report: AudioArtifactReference,
    pub timing_authority: AlignmentTimingAuthority,
    pub conversion: FileConversionOutcome,
}

/// Export a validated candidate without changing the original reviewed text or
/// claiming its new timing was reviewed. Partial cues remain untimed; formats
/// that require intervals refuse them. Every conversion loss needs permission.
pub fn export_alignment_file(
    input: &Path,
    to: TimedTextFormat,
    output_directory: &Path,
    options: &ConversionOptions,
) -> Result<AlignmentExportOutcome, ConversionFailure> {
    let (mut alignment_report, conversion) =
        convert_embedded_file(input, to, output_directory, options, |bytes| {
            let report = AudioAlignmentReport::from_json_slice(bytes)?;
            report.timed_text.ok_or_else(|| {
                crate::Error::new(
                    crate::ErrorCategory::Configuration,
                    "alignment report has no candidate text to export",
                )
            })
        })?;
    alignment_report.id = "alignment_report".into();
    Ok(AlignmentExportOutcome {
        alignment_report,
        timing_authority: AlignmentTimingAuthority::Candidate,
        conversion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_alignment_cannot_reserve_an_export_directory() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("report.json");
        let output = root.path().join("export");
        let bytes = b"{\"schema\":\"untrusted\",\"timed_text\":null}";
        std::fs::write(&input, bytes).unwrap();
        assert!(
            export_alignment_file(
                &input,
                TimedTextFormat::Srt,
                &output,
                &ConversionOptions::default()
            )
            .is_err()
        );
        assert!(!output.exists());
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
    }
}
