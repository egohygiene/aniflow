//! Fresh manually supplied lyric revisions. No review is transferred/generated.
use super::*;
use crate::destination::local;
use std::collections::BTreeMap;
const REQUEST_SCHEMA: &str = "aniflow.manual-lyrics-revision-request/v1";
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        Err(invalid("manual lyric revision cancelled"))
    } else {
        Ok(())
    }
}
fn absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.as_os_str().len() <= 4096
        && !path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionRequest {
    pub schema: String,
    pub lyrics_request: ManualLyricsRequest,
    pub previous_run_directory: PathBuf,
    pub expected_worksheet_sha256: String,
    pub new_input: PathBuf,
    pub expected_input_sha256: String,
    pub operator: String,
    pub declared_origin: String,
}
impl RevisionRequest {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 131_072 {
            return Err(invalid("lyric revision request exceeds 128 KiB"));
        }
        let value: Self = decode_json(bytes, "manual lyric revision request")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        for pin in [&self.expected_worksheet_sha256, &self.expected_input_sha256] {
            require_sha256(pin, "lyric revision binding")?;
        }
        if self.schema != REQUEST_SCHEMA
            || !absolute(&self.previous_run_directory)
            || !absolute(&self.new_input)
            || self.operator.trim().is_empty()
            || self.operator.len() > 256
            || self.operator.chars().any(char::is_control)
            || self.declared_origin.trim().is_empty()
            || self.declared_origin.len() > 2048
            || self.declared_origin.chars().any(char::is_control)
            || canonical_json_bytes(self)?.len() > 131_072
        {
            return Err(invalid(
                "revision needs exact explicit parent/input and bounded operator-declared origin",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueChanges {
    pub added: u64,
    pub removed: u64,
    pub shared: u64,
    pub text_changed: u64,
    pub timing_changed: u64,
    pub label_or_speaker_changed: u64,
    pub correspondence: String,
}
fn changes(before: &TimedTextDocument, after: &TimedTextDocument) -> Result<CueChanges> {
    before.validate()?;
    after.validate()?;
    let old = before
        .cues
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect::<BTreeMap<_, _>>();
    let new = after
        .cues
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect::<BTreeMap<_, _>>();
    let mut result = CueChanges {
        correspondence: "native cue ID only; no inferred positional/text alignment".into(),
        ..CueChanges::default()
    };
    for (id, cue) in &old {
        if let Some(next) = new.get(id) {
            result.shared += 1;
            result.text_changed += u64::from(cue.text != next.text);
            result.timing_changed += u64::from(cue.timing != next.timing);
            result.label_or_speaker_changed +=
                u64::from(cue.source_label != next.source_label || cue.speaker != next.speaker);
        } else {
            result.removed += 1;
        }
    }
    result.added = new.keys().filter(|id| !old.contains_key(**id)).count() as u64;
    Ok(result)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionPlan {
    pub schema: String,
    pub intent_sha256: String,
    pub request: RevisionRequest,
    pub previous_snapshot_sha256: String,
    pub new_input_byte_count: u64,
    pub new_document_sha256: String,
    pub changes: CueChanges,
    pub review_policy: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionReceipt {
    pub schema: String,
    pub intent_sha256: String,
    pub previous_worksheet_sha256: String,
    pub previous_snapshot_sha256: String,
    pub new_input_sha256: String,
    pub new_document_sha256: String,
    pub new_input_relative_path: String,
    pub new_workflow_relative_path: String,
    pub words_review_transferred: bool,
    pub timing_review_transferred: bool,
    pub approval_authority: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionStatus {
    pub schema: String,
    pub receipt_sha256: String,
    pub intent_sha256: String,
    pub previous_worksheet_sha256: String,
    pub changes: CueChanges,
    pub current: ManualLyricsStatus,
    pub parent_observation: String,
    pub approval_authority: String,
}
fn intent_hash(plan: &RevisionPlan) -> Result<String> {
    let mut value = plan.clone();
    value.intent_sha256.clear();
    canonical_sha256(&value)
}
fn current_new(
    request: &ManualLyricsRequest,
    root: &Path,
    input_sha256: &str,
    document_sha256: &str,
) -> Result<ManualLyricsStatus> {
    local::entries(
        root,
        &["request.json", "audio-relative.json"],
        &["words", "timing"],
        true,
    )?;
    let current = super::status(request, root)?;
    let bytes = super::read_bytes(
        &root.join("words/artifact"),
        timed_text::MAX_TEXT_BYTES as u64,
    )?;
    let document = super::decode(request, &bytes)?;
    if sha(&bytes) != input_sha256
        || canonical_sha256(&document)? != document_sha256
        || current.words.subject_sha256.as_deref() != Some(input_sha256)
        || current.timing.subject_sha256.as_deref() != Some(input_sha256)
        || super::read_bytes(
            &root.join("timing/artifact"),
            timed_text::MAX_TEXT_BYTES as u64,
        )? != bytes
        || super::read_bytes(
            &root.join("audio-relative.json"),
            timed_text::MAX_EVIDENCE_BYTES,
        )? != document.canonical_json_bytes()?
    {
        return Err(invalid(
            "revised content differs from its exact imported source/document",
        ));
    }
    Ok(current)
}
/// Bind a fresh current worksheet and a separately supplied valid interval file.
/// Source audio/clock/offset/configuration remain the exact existing request.
pub fn plan(request: &RevisionRequest, token: &CancellationToken) -> Result<RevisionPlan> {
    cancelled(token)?;
    request.validate()?;
    let previous = worksheet::observe(
        &request.lyrics_request,
        &request.previous_run_directory,
        token,
    )?;
    if previous.worksheet_sha256 != request.expected_worksheet_sha256 {
        return Err(invalid("prior cue worksheet/review state changed"));
    }
    let snapshot = local::bytes(&previous)?;
    let bytes = super::read_bytes(&request.new_input, timed_text::MAX_TEXT_BYTES as u64)?;
    if sha(&bytes) != request.expected_input_sha256 || sha(&bytes) == previous.payload.input_sha256
    {
        return Err(invalid(
            "revision requires changed exact supplied interval bytes",
        ));
    }
    let document = super::decode(&request.lyrics_request, &bytes)?;
    let delta = changes(&previous.payload.audio_relative, &document)?;
    if worksheet::observe(
        &request.lyrics_request,
        &request.previous_run_directory,
        token,
    )? != previous
        || super::read_bytes(&request.new_input, timed_text::MAX_TEXT_BYTES as u64)? != bytes
    {
        return Err(invalid("parent/input changed while planning revision"));
    }
    let mut output=RevisionPlan {schema:"aniflow.manual-lyrics-revision-plan/v1".into(),intent_sha256:String::new(),request:request.clone(),previous_snapshot_sha256:local::hash(&snapshot),new_input_byte_count:bytes.len() as u64,new_document_sha256:canonical_sha256(&document)?,changes:delta,
        review_policy:"new ordinary lyric workflow; words and timing both require fresh exact decisions; no render approval".into()};
    output.intent_sha256 = intent_hash(&output)?;
    local::bytes(&output)?;
    cancelled(token)?;
    Ok(output)
}
/// Initialize a new ordinary lyrics workspace. Preserve the previous workflow
/// unchanged, and publish the revision receipt only after exact new import.
pub fn create(
    request: &RevisionRequest,
    root: &Path,
    token: &CancellationToken,
) -> Result<RevisionStatus> {
    cancelled(token)?;
    local::parent(root)?;
    for input in [
        &request.previous_run_directory,
        &request.new_input,
        &request.lyrics_request.audio_file,
    ] {
        if root.starts_with(input) || input.starts_with(root) {
            return Err(invalid(
                "revision root must be separate from previous workflow and original inputs",
            ));
        }
    }
    let expected = plan(request, token)?;
    let previous = worksheet::observe(
        &request.lyrics_request,
        &request.previous_run_directory,
        token,
    )?;
    if local::hash(&local::bytes(&previous)?) != expected.previous_snapshot_sha256 {
        return Err(invalid(
            "previous worksheet changed before revision creation",
        ));
    }
    cancelled(token)?;
    local::directory(root)?;
    local::write(
        &root.join("revision-intent.json"),
        &local::bytes(&expected)?,
    )?;
    local::write(
        &root.join("previous-worksheet.json"),
        &local::bytes(&previous)?,
    )?;
    let bytes = super::read_bytes(&request.new_input, timed_text::MAX_TEXT_BYTES as u64)?;
    if sha(&bytes) != request.expected_input_sha256
        || bytes.len() as u64 != expected.new_input_byte_count
    {
        return Err(invalid("supplied revision changed before snapshot"));
    }
    let snapshot = root.join("input.bin");
    local::write(&snapshot, &bytes)?;
    let new_root = root.join("lyrics");
    super::initialize(&request.lyrics_request, &new_root, Some(&snapshot), token)?;
    let current = current_new(
        &request.lyrics_request,
        &new_root,
        &request.expected_input_sha256,
        &expected.new_document_sha256,
    )?;
    if current.words.state != ManualState::AwaitingReview
        || current.timing.state != ManualState::AwaitingReview
        || current.ready_for_sidecar_export
    {
        return Err(invalid("new revision cannot inherit a prior decision"));
    }
    if plan(request, token)? != expected {
        return Err(invalid(
            "parent/input changed while creating revision; preserve incomplete evidence",
        ));
    }
    let receipt = RevisionReceipt {
        schema: "aniflow.manual-lyrics-revision-receipt/v1".into(),
        intent_sha256: expected.intent_sha256,
        previous_worksheet_sha256: request.expected_worksheet_sha256.clone(),
        previous_snapshot_sha256: expected.previous_snapshot_sha256,
        new_input_sha256: request.expected_input_sha256.clone(),
        new_document_sha256: expected.new_document_sha256,
        new_input_relative_path: "input.bin".into(),
        new_workflow_relative_path: "lyrics".into(),
        words_review_transferred: false,
        timing_review_transferred: false,
        approval_authority: "existing new lyric owner only; no review generated or transferred"
            .into(),
    };
    cancelled(token)?;
    let bytes = local::bytes(&receipt)?;
    local::write(&root.join("receipt.json"), &bytes)?;
    observe(root, &local::hash(&bytes), token)
}
/// Retained parent bytes and current new-owner reviews are distinct. The old
/// workspace need not remain present; original bound audio is still required.
pub fn observe(
    root: &Path,
    receipt_sha256: &str,
    token: &CancellationToken,
) -> Result<RevisionStatus> {
    cancelled(token)?;
    require_sha256(receipt_sha256, "lyric revision receipt")?;
    local::entries(
        root,
        &[
            "revision-intent.json",
            "previous-worksheet.json",
            "input.bin",
            "receipt.json",
        ],
        &["lyrics"],
        false,
    )?;
    let receipt_path = root.join("receipt.json");
    let receipt_pin = identity(&receipt_path, local::DOCUMENT_BYTES)?;
    if receipt_pin.0 != receipt_sha256 {
        return Err(invalid("revision receipt differs from external pin"));
    }
    let receipt: RevisionReceipt = local::read(&receipt_path)?;
    let intent: RevisionPlan = local::read(&root.join("revision-intent.json"))?;
    let previous: worksheet::CueWorksheet = local::read(&root.join("previous-worksheet.json"))?;
    intent.request.validate()?;
    if intent.schema != "aniflow.manual-lyrics-revision-plan/v1"
        || intent_hash(&intent)? != intent.intent_sha256
        || receipt.schema != "aniflow.manual-lyrics-revision-receipt/v1"
        || receipt.intent_sha256 != intent.intent_sha256
        || receipt.previous_worksheet_sha256 != intent.request.expected_worksheet_sha256
        || receipt.previous_snapshot_sha256 != intent.previous_snapshot_sha256
        || receipt.previous_snapshot_sha256 != local::hash(&local::bytes(&previous)?)
        || previous.schema != "aniflow.lyric-cue-worksheet/v1"
        || previous.worksheet_sha256 != canonical_sha256(&previous.payload)?
        || previous.worksheet_sha256 != receipt.previous_worksheet_sha256
        || previous.payload.request_sha256 != canonical_sha256(&intent.request.lyrics_request)?
        || receipt.new_input_sha256 != intent.request.expected_input_sha256
        || receipt.new_document_sha256 != intent.new_document_sha256
        || receipt.new_input_relative_path != "input.bin"
        || receipt.new_workflow_relative_path != "lyrics"
        || receipt.words_review_transferred
        || receipt.timing_review_transferred
        || receipt.approval_authority
            != "existing new lyric owner only; no review generated or transferred"
        || intent.review_policy
            != "new ordinary lyric workflow; words and timing both require fresh exact decisions; no render approval"
    {
        return Err(invalid(
            "revision intent/parent/import/review authority differs",
        ));
    }
    let input_pin = identity(&root.join("input.bin"), timed_text::MAX_TEXT_BYTES as u64)?;
    if input_pin
        != (
            receipt.new_input_sha256.clone(),
            intent.new_input_byte_count,
        )
    {
        return Err(invalid("retained revision input changed"));
    }
    let new_root = root.join("lyrics");
    let current = current_new(
        &intent.request.lyrics_request,
        &new_root,
        &receipt.new_input_sha256,
        &receipt.new_document_sha256,
    )?;
    let document = super::decode(
        &intent.request.lyrics_request,
        &super::read_bytes(
            &new_root.join("words/artifact"),
            timed_text::MAX_TEXT_BYTES as u64,
        )?,
    )?;
    if document.source.byte_size != intent.new_input_byte_count
        || changes(&previous.payload.audio_relative, &document)? != intent.changes
    {
        return Err(invalid(
            "revision cue changes or input extent differ from retained exact documents",
        ));
    }
    if current_new(
        &intent.request.lyrics_request,
        &new_root,
        &receipt.new_input_sha256,
        &receipt.new_document_sha256,
    )? != current
        || local::read::<RevisionPlan>(&root.join("revision-intent.json"))? != intent
        || local::read::<worksheet::CueWorksheet>(&root.join("previous-worksheet.json"))?
            != previous
        || identity(&root.join("input.bin"), timed_text::MAX_TEXT_BYTES as u64)? != input_pin
        || identity(&receipt_path, local::DOCUMENT_BYTES)? != receipt_pin
    {
        return Err(invalid(
            "revision/current review changed during observation",
        ));
    }
    local::entries(
        root,
        &[
            "revision-intent.json",
            "previous-worksheet.json",
            "input.bin",
            "receipt.json",
        ],
        &["lyrics"],
        false,
    )?;
    cancelled(token)?;
    Ok(RevisionStatus {
        schema: "aniflow.manual-lyrics-revision-status/v1".into(),
        receipt_sha256: receipt_sha256.into(),
        intent_sha256: intent.intent_sha256,
        previous_worksheet_sha256: receipt.previous_worksheet_sha256,
        changes: intent.changes,
        current,
        parent_observation: "retained exact parent worksheet; previous workspace not reobserved"
            .into(),
        approval_authority: receipt.approval_authority,
    })
}
#[cfg(test)]
mod tests;
