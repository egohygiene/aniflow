//! Delivery audio receipts and views over existing #13 signal evidence.
//! No analyzer, normalizer, source rewrite or new acceptance authority.
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::temporal::{RationalTime, TemporalInspection, TimeRange};
use crate::{Error, ErrorCategory, Result};

fn invalid(message: &str) -> Error { Error::new(ErrorCategory::Media,message) }
fn mapped(error: anyhow::Error) -> Error { Error::from_anyhow(ErrorCategory::Media,error) }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTimingReceipt {
    pub schema: String,
    pub video: TimeRange,
    pub source_audio: TimeRange,
    pub output_audio: TimeRange,
    pub audio_start_offset: RationalTime,
    /// Signed difference: positive means the song continues beyond the picture.
    pub audio_end_offset: RationalTime,
    pub exact_extent_preserved: bool,
    pub normalization: String,
    pub loudness: String,
}

pub(crate) fn audio_timing(source: &TemporalInspection, output: &TemporalInspection) -> Result<Option<AudioTimingReceipt>> {
    let audio_range = |report: &TemporalInspection| report.selected.as_ref().and_then(|s| s.audio)
        .and_then(|i| report.timeline(i)).and_then(|t| t.source_time);
    let Some(original) = audio_range(source) else {
        if audio_range(output).is_some() { return Err(invalid("output added unselected audio")); }
        return Ok(None);
    };
    let actual = audio_range(output).ok_or_else(|| invalid("output lost selected audio"))?;
    let video = source.video_timeline().map_err(mapped)?.source_time.ok_or_else(|| invalid("video extent is missing"))?;
    Ok(Some(AudioTimingReceipt {
        schema:"aniflow.delivery-audio-timing/v1".to_owned(),video,source_audio:original,output_audio:actual,
        audio_start_offset:original.start.checked_sub(video.start).map_err(mapped)?,
        audio_end_offset:original.end.checked_sub(video.end).map_err(mapped)?,
        exact_extent_preserved:original == actual,
        normalization:"none".to_owned(),
        loudness:"not_measured_by_assembly; consume separately accepted #13 signal evidence".to_owned(),
    }))
}

/// A view of the measured PCM subject. This does not assert its relationship to
/// compressed video audio; that relationship needs separate exact decode evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryLoudnessSummary {
    pub schema: String,
    pub report_sha256: String,
    pub report_schema: String,
    pub source: crate::audio_analysis::AudioSource,
    pub integrated_loudness: crate::audio_signal::AudioSignalMeasurement,
    pub true_peak: crate::audio_signal::AudioSignalMeasurement,
    pub normalization: String,
    pub video_audio_binding: String,
}

/// Read only a complete, currently accepted signal run. Native v3 unavailable
/// measurements retain their typed reasons; no missing quantity is invented.
pub fn loudness_summary(run_directory: impl AsRef<Path>) -> Result<DeliveryLoudnessSummary> {
    use std::io::Read;
    use sha2::{Digest,Sha256};
    let root = run_directory.as_ref();
    if root.canonicalize().ok().as_deref() != Some(root) { return Err(invalid("signal run must be canonical")); }
    let manifest = crate::status_v3(root)?;
    if manifest.payload.state != crate::PipelineV3RunState::Complete || manifest.payload.delivery.is_none() {
        return Err(invalid("signal evidence requires complete current acceptance"));
    }
    let mut outputs = manifest.payload.outputs.iter().filter(|v| v.id == "signal");
    let output = outputs.next().ok_or_else(|| invalid("accepted signal port is missing"))?;
    if outputs.next().is_some() { return Err(invalid("duplicate signal port")); }
    output.validate()?;
    let path = root.join(output.relative_path.as_ref().ok_or_else(|| invalid("signal port has no owned locator"))?);
    if path.canonicalize().ok().as_deref() != Some(path.as_path()) || !path.starts_with(root) { return Err(invalid("signal port escaped its accepted workspace")); }
    let mut bytes = Vec::new();
    crate::audio_inspection::process::open_regular(&path).map_err(|_| invalid("cannot open signal report"))?
        .take(8_388_609).read_to_end(&mut bytes).map_err(|_| invalid("cannot read bounded signal report"))?;
    let hash = format!("{:x}",Sha256::digest(&bytes));
    if bytes.len() > 8_388_608 || hash != output.sha256 || bytes.len() as u64 != output.byte_count { return Err(invalid("accepted signal report changed or exceeds its bound")); }
    let header: serde_json::Value = crate::provider::decode_json(&bytes,"signal report")?;
    let (report_schema,source,integrated_loudness,true_peak) = match header.get("schema").and_then(serde_json::Value::as_str) {
        Some(crate::audio_signal::AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V3) => {
            let value = crate::audio_signal::NativeAudioSignalMeasurements::from_json_slice(&bytes)?;
            (value.schema,value.source,value.integrated_loudness,value.true_peak)
        },
        _ => {
            let value = crate::audio_signal::AudioSignalMeasurements::from_json_slice(&bytes)?;
            (value.schema,value.source,value.integrated_loudness,value.true_peak)
        },
    };
    if crate::status_v3(root)? != manifest { return Err(invalid("signal acceptance changed during observation")); }
    Ok(DeliveryLoudnessSummary { schema:"aniflow.delivery-loudness-summary/v1".to_owned(),report_sha256:hash,
        report_schema,source,integrated_loudness,true_peak,normalization:"none".to_owned(),
        video_audio_binding:"not_established_by_this_view".to_owned() })
}
