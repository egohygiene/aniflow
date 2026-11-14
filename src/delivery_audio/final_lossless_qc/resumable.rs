//! Fixed restart boundaries over existing native decode, v3 signal and meter
//! owners. Markers locate accepted native work; they never accept media.
use super::*;
use crate::PipelineV3Plan;
mod execution;
pub use execution::{resume, run};
#[cfg(test)]
mod tests;
const ALLOWANCE: u64 = 16_777_216;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumableQcRequest {
    pub schema: String,
    pub qc: FinalLosslessQcRequest,
}
impl ResumableQcRequest {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 {
            return Err(invalid("resumable final-QC request exceeds 64 KiB"));
        }
        let value: Self = crate::provider::decode_json(bytes, "resumable final-QC request")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        self.qc.validate()?;
        if self.schema != "aniflow.resumable-final-audio-qc-request/v1"
            || self.qc.schema != "aniflow.final-lossless-audio-qc-request/v2"
        {
            return Err(invalid(
                "resumable final QC requires its explicit wrapper and final-QC v2; legacy roots are not upgraded",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumableQcIntent {
    pub schema: String,
    pub request: ResumableQcRequest,
    pub native: FinalLosslessQcIntent,
    pub projected_working_bytes: u64,
    pub intent_sha256: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumableQcPhase {
    Capture,
    Decode,
    NativeSignal,
    Meter,
    Complete,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumableQcStatus {
    pub schema: String,
    pub run_directory: PathBuf,
    pub intent_sha256: String,
    pub phase: ResumableQcPhase,
    pub signal_plan_sha256: Option<String>,
    pub signal_manifest_sha256: Option<String>,
    pub report: Option<FinalLosslessQcReport>,
    pub complete: bool,
    pub next_action: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    schema: String,
    intent_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Decoded {
    schema: String,
    intent_sha256: String,
    wav_sha256: String,
    wav_bytes: u64,
    plan: PipelineV3Plan,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Meter {
    schema: String,
    intent_sha256: String,
    report: FinalLosslessQcReport,
}
fn digest(intent: &ResumableQcIntent) -> Result<String> {
    crate::provider::canonical_sha256(&(
        &intent.schema,
        &intent.request,
        &intent.native,
        intent.projected_working_bytes,
    ))
}
/// Current accepted source and component identities only; no process or writes.
pub fn plan(request: &ResumableQcRequest, token: &CancellationToken) -> Result<ResumableQcIntent> {
    request.validate()?;
    let native = super::plan(&request.qc, token)?;
    let projected_working_bytes = native
        .projected_working_bytes
        .checked_add(ALLOWANCE)
        .ok_or_else(|| invalid("resumable QC budget overflow"))?;
    if projected_working_bytes > request.qc.maximum_working_bytes {
        return Err(invalid(
            "resumable QC metadata allowance exceeds selected whole working bound",
        ));
    }
    let mut value = ResumableQcIntent {
        schema: "aniflow.resumable-final-audio-qc-intent/v1".into(),
        request: request.clone(),
        native,
        projected_working_bytes,
        intent_sha256: String::new(),
    };
    value.intent_sha256 = digest(&value)?;
    local::bytes(&value)?;
    cancelled(token)?;
    Ok(value)
}
fn present(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(value) if value.is_file() || value.is_dir() => Ok(true),
        Ok(_) => Err(invalid(
            "resumable QC member is symbolic or special; preserve it for reconciliation",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(invalid("cannot inspect resumable QC member")),
    }
}
pub(crate) fn recognized(root: &Path) -> Result<bool> {
    present(&root.join("resume-intent.json"))
}
fn child(root: &Path) -> PathBuf {
    root.join("children/signal")
}
fn retained(root: &Path, token: &CancellationToken) -> Result<ResumableQcIntent> {
    cancelled(token)?;
    ensure_canonical_directory(root)?;
    let intent: ResumableQcIntent = local::read(&root.join("resume-intent.json"))?;
    if intent.schema != "aniflow.resumable-final-audio-qc-intent/v1"
        || intent.intent_sha256 != digest(&intent)?
        || plan(&intent.request, token)? != intent
    {
        return Err(invalid(
            "resumable QC intent, current source acceptance, settings or components changed; preserve this root and use a new intent",
        ));
    }
    runtime::subject_disjoint(root, &intent.native)?;
    Ok(intent)
}
fn capture_files(root: &Path, intent: &ResumableQcIntent) -> Result<()> {
    if identity(
        &root.join("source.media"),
        crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES,
    )? != (
        intent.native.source.file.sha256.clone(),
        intent.native.source.file.byte_count,
    ) {
        return Err(invalid("resumable QC private source snapshot changed"));
    }
    for (component, name) in
        intent
            .native
            .components
            .iter()
            .zip(["ffmpeg", "ffprobe", "aniflow-provider"])
    {
        let pin = (component.sha256.clone(), component.byte_count);
        if identity(&component.path, 134_217_728)? != pin
            || identity(&root.join(name), 134_217_728)? != pin
        {
            return Err(invalid("resumable QC original/private component changed"));
        }
    }
    Ok(())
}
fn accepted_report(
    root: &Path,
    intent: &ResumableQcIntent,
    decoded: &Decoded,
    report: &FinalLosslessQcReport,
    token: &CancellationToken,
) -> Result<()> {
    let native = &intent.native;
    let accepted = crate::status_v3(&child(root))?;
    if accepted.payload.state != crate::PipelineV3RunState::Complete
        || accepted.payload.delivery.is_none()
        || accepted.payload.plan_sha256 != decoded.plan.plan_sha256
        || report.signal_run_directory != child(root)
        || report.signal_plan_sha256 != accepted.payload.plan_sha256
        || report.signal_manifest_sha256 != accepted.manifest_sha256
    {
        return Err(invalid(
            "resumable QC meter/report lacks exact current complete native signal acceptance",
        ));
    }
    let summary = super::super::loudness_summary(&child(root))?;
    let pin = wave(root, native)?;
    runtime::bind_summary(native, &pin, &summary)?;
    let (_, encoding, pcm) = source_facts(&native.source)?;
    if report.schema != "aniflow.final-lossless-audio-qc/v2"
        || report.intent != *native
        || report.intent_sha256 != crate::provider::canonical_sha256(native)?
        || report.decoded_wav_sha256 != pin.0
        || report.decoded_wav_bytes != pin.1
        || report.decoded_pcm_sha256 != pcm
        || report.encoding != encoding
        || report.accepted_pcm != summary
        || report.normalization != "none"
        || report.runtime_qualified
        || report.creative_approval
            != "not_performed; final-file measurements are not musical quality review"
        || report.publication_authority != "none"
    {
        return Err(invalid(
            "resumable QC retained meter/report differs from exact final subject or native evidence",
        ));
    }
    runtime::validate_meter(native, &summary, report)?;
    runtime::recheck_files(root, native)?;
    if crate::status_v3(&child(root))? != accepted {
        return Err(invalid(
            "native signal acceptance changed during resumable QC readback",
        ));
    }
    cancelled(token)
}
/// Read-only transition projection. No tool preflight, decoder, provider or meter
/// executes. Partial unmarked capture/decode/native initialization is refused.
pub fn status(root: &Path, token: &CancellationToken) -> Result<ResumableQcStatus> {
    let intent = retained(root, token)?;
    let mut files = vec!["resume-intent.json"];
    ensure_canonical_directory(&root.join("children"))?;
    let capture = present(&root.join("capture.json"))?;
    let decode = present(&root.join("decoded.json"))?;
    let native = present(&child(root))?;
    let meter = present(&root.join("meter.json"))?;
    let complete = present(&root.join("qc.json"))?;
    if !capture && (decode || native || meter || complete)
        || !decode && (native || meter || complete)
        || !meter && complete
    {
        return Err(invalid(
            "resumable QC successor exists before its committed predecessor; retain for reconciliation",
        ));
    }
    let mut phase = ResumableQcPhase::Capture;
    let mut signal_plan_sha256 = None;
    let mut signal_manifest_sha256 = None;
    let mut report = None;
    if capture {
        let saved: Capture = local::read(&root.join("capture.json"))?;
        if saved != capture_marker(&intent) {
            return Err(invalid(
                "resumable QC capture marker differs from immutable intent",
            ));
        }
        capture_files(root, &intent)?;
        files.extend([
            "capture.json",
            "source.media",
            "ffmpeg",
            "ffprobe",
            "aniflow-provider",
        ]);
        phase = ResumableQcPhase::Decode;
    }
    if decode {
        let saved: Decoded = local::read(&root.join("decoded.json"))?;
        let pin = wave(root, &intent.native)?;
        let expected =
            crate::audio_signal::plan_read_only(&signal_request(root, &intent.native), token)?;
        if saved.schema != "aniflow.resumable-final-audio-qc-decoded/v1"
            || saved.intent_sha256 != intent.intent_sha256
            || (saved.wav_sha256.clone(), saved.wav_bytes) != pin
            || saved.plan != expected
        {
            return Err(invalid(
                "resumable QC decoded marker/complete samples/exact current native plan changed",
            ));
        }
        runtime::recheck_files(root, &intent.native)?;
        files.extend(["decoded.json", "decoded.wav"]);
        if flac(&intent.native) {
            files.push("decoded.pcm");
        }
        signal_plan_sha256 = Some(saved.plan.plan_sha256.clone());
        phase = ResumableQcPhase::NativeSignal;
        if native {
            crate::audio_inspection::require_same_plan(&child(root), &saved.plan)?;
            let accepted = crate::status_v3(&child(root))?;
            if accepted.payload.plan_sha256 != saved.plan.plan_sha256 {
                return Err(invalid("resumable QC native manifest/plan mismatch"));
            }
            signal_manifest_sha256 = Some(accepted.manifest_sha256.clone());
            if accepted.payload.state == crate::PipelineV3RunState::Complete
                && accepted.payload.delivery.is_some()
            {
                phase = ResumableQcPhase::Meter;
            }
        }
        if meter {
            if phase != ResumableQcPhase::Meter {
                return Err(invalid(
                    "resumable QC meter precedes accepted complete native signal",
                ));
            }
            let saved_meter: Meter = local::read(&root.join("meter.json"))?;
            if saved_meter.schema != "aniflow.resumable-final-audio-qc-meter/v1"
                || saved_meter.intent_sha256 != intent.intent_sha256
            {
                return Err(invalid("resumable QC meter marker binding changed"));
            }
            accepted_report(root, &intent, &saved, &saved_meter.report, token)?;
            files.push("meter.json");
            if complete {
                let actual: FinalLosslessQcReport = local::read(&root.join("qc.json"))?;
                if actual != saved_meter.report {
                    return Err(invalid(
                        "resumable QC final receipt differs from retained exact meter observation",
                    ));
                }
                files.push("qc.json");
                report = Some(actual);
                phase = ResumableQcPhase::Complete;
            }
        }
    }
    local::entries(root, &files, &["children"], true)?;
    local::entries(
        &root.join("children"),
        &[],
        &if native { vec!["signal"] } else { vec![] },
        false,
    )?;
    if retained(root, token)? != intent {
        return Err(invalid(
            "resumable QC source/components changed during status",
        ));
    }
    Ok(ResumableQcStatus {
        schema: "aniflow.resumable-final-audio-qc-status/v1".into(),
        run_directory: root.to_path_buf(),
        intent_sha256: intent.intent_sha256,
        phase,
        signal_plan_sha256,
        signal_manifest_sha256,
        report,
        complete,
        next_action: if complete {
            "complete"
        } else {
            "resume_recognized_qc"
        }
        .into(),
    })
}
fn capture_marker(intent: &ResumableQcIntent) -> Capture {
    Capture {
        schema: "aniflow.resumable-final-audio-qc-capture/v1".into(),
        intent_sha256: intent.intent_sha256.clone(),
    }
}
/// Existing final receipt pin and report shape; every transition/current native
/// authority is rechecked before this can feed destination preflight.
pub fn observe(
    root: &Path,
    receipt_sha256: &str,
    token: &CancellationToken,
) -> Result<FinalLosslessQcReport> {
    crate::provider::require_sha256(receipt_sha256, "resumable final-QC receipt")?;
    let before = identity(&root.join("qc.json"), local::DOCUMENT_BYTES)?;
    if before.0 != receipt_sha256 {
        return Err(invalid("resumable QC receipt differs from external digest"));
    }
    let current = status(root, token)?;
    let report = current
        .report
        .ok_or_else(|| invalid("resumable QC is not complete"))?;
    if identity(&root.join("qc.json"), local::DOCUMENT_BYTES)? != before {
        return Err(invalid(
            "resumable final-QC receipt changed during readback",
        ));
    }
    Ok(report)
}
