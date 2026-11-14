//! Closed manual input/review checkpoints for music-video work. Decisions are
//! operator declarations about exact bytes, never native acceptance or identity.
use crate::frame_io::{copy_pinned, ensure_canonical_directory, identity, require_absent};
use crate::provider::{canonical_json_bytes, canonical_sha256, decode_json, require_sha256};
use crate::{CancellationToken, Error, ErrorCategory, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAXIMUM_INPUT: u64 = 8_388_608;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualRole {
    PreparedFrames,
    TimedLyrics,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualContext {
    pub schema: String,
    pub role: ManualRole,
    pub source_sha256: String,
    pub timeline_sha256: String,
    pub configuration_sha256: String,
    pub recipe_intent_sha256: String,
}
impl ManualContext {
    pub fn validate(&self) -> Result<()> {
        if self.schema != "aniflow.manual-context/v1" {
            return Err(invalid("unsupported manual context"));
        }
        for hash in [
            &self.source_sha256,
            &self.timeline_sha256,
            &self.configuration_sha256,
            &self.recipe_intent_sha256,
        ] {
            require_sha256(hash, "manual context binding")?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    sha256: String,
    byte_count: u64,
    original_path: Option<PathBuf>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Approve,
    Reject,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualReview {
    pub schema: String,
    pub context_sha256: String,
    pub subject_sha256: String,
    pub decision: ReviewDecision,
    pub reviewer: String,
    pub notes: String,
}
impl ManualReview {
    /// Validate a supplied generic declaration; this grants no approval and
    /// does not validate its domain-specific context or subject binding.
    pub fn validate(&self) -> Result<()> {
        validate_review(self)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualState {
    AwaitingInput,
    AwaitingReview,
    Approved,
    Rejected,
    Invalidated,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualStatus {
    pub schema: String,
    pub context_sha256: String,
    pub subject_sha256: Option<String>,
    pub state: ManualState,
    pub reason: String,
    pub review_is_declared: bool,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::State, message)
}
fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(invalid("manual checkpoint entry cannot be inspected")),
    }
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let expected = identity(path, 65_536)?;
    let mut bytes = Vec::new();
    crate::audio_inspection::process::open_regular(path)
        .map_err(|_| invalid("manual record unavailable"))?
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("manual record unreadable"))?;
    if bytes.len() as u64 != expected.1
        || format!("{:x}", Sha256::digest(&bytes)) != expected.0
        || identity(path, 65_536)? != expected
    {
        return Err(invalid("manual record changed during observation"));
    }
    decode_json(&bytes, "manual checkpoint record")
}
fn write<T: Serialize>(root: &Path, name: &str, value: &T) -> Result<()> {
    let bytes = canonical_json_bytes(value)?;
    if bytes.len() > 65_536 {
        return Err(invalid("manual record exceeds 64 KiB"));
    }
    let mut file = tempfile::NamedTempFile::new_in(root)
        .map_err(|_| invalid("cannot reserve private manual record"))?;
    file.write_all(&bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|_| invalid("cannot sync manual record"))?;
    file.persist_noclobber(root.join(name))
        .map_err(|_| invalid("manual record already exists or cannot be published"))?;
    fs::File::open(root)
        .and_then(|f| f.sync_all())
        .map_err(|_| invalid("cannot sync manual checkpoint directory"))?;
    Ok(())
}
fn layout(root: &Path) -> Result<()> {
    ensure_canonical_directory(root)?;
    let mut count = 0;
    for entry in fs::read_dir(root).map_err(|_| invalid("manual checkpoint unavailable"))? {
        count += 1;
        if count > 5 {
            return Err(invalid("manual checkpoint contains too many entries"));
        }
        let path = entry
            .map_err(|_| invalid("manual checkpoint unreadable"))?
            .path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| invalid("manual entry name invalid"))?;
        if !matches!(
            name,
            "context.json" | "subject.json" | "artifact" | "review.json" | "writer.lock"
        ) || !fs::symlink_metadata(&path)
            .map_err(|_| invalid("manual entry unavailable"))?
            .is_file()
            || path.canonicalize().ok().as_deref() != Some(path.as_path())
        {
            return Err(invalid(
                "manual checkpoint has an unexpected or unsafe entry; preserve it for review",
            ));
        }
    }
    Ok(())
}

/// Create a fresh checkpoint with no input or review inferred.
pub fn create(root: &Path, context: &ManualContext) -> Result<ManualStatus> {
    context.validate()?;
    ensure_canonical_directory(
        root.parent()
            .ok_or_else(|| invalid("manual checkpoint needs an existing parent"))?,
    )?;
    require_absent(root)?;
    fs::create_dir(root).map_err(|_| invalid("cannot reserve manual checkpoint"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))
            .map_err(|_| invalid("cannot make manual checkpoint private"))?;
    }
    ensure_canonical_directory(root)?;
    write(root, "context.json", context)?;
    status(root, context)
}

/// Attach one exact manually authored file; later edits invalidate its approval.
/// Timed-text parsing and semantic timing approval remain the #108 owner.
pub fn attach_file(
    root: &Path,
    expected: &ManualContext,
    input: &Path,
    token: &CancellationToken,
) -> Result<ManualStatus> {
    expected.validate()?;
    layout(root)?;
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let saved: ManualContext = read(&root.join("context.json"))?;
    if saved != *expected
        || saved.role != ManualRole::TimedLyrics
        || exists(&root.join("subject.json"))?
        || exists(&root.join("review.json"))?
        || input.starts_with(root)
    {
        return Err(invalid(
            "manual attachment requires the unchanged empty timed-lyrics checkpoint and an external source",
        ));
    }
    let value = identity(input, MAXIMUM_INPUT)?;
    if value.1 == 0 {
        return Err(invalid("manual input cannot be empty"));
    }
    copy_pinned(input, &root.join("artifact"), &value, MAXIMUM_INPUT, token)?;
    write(
        root,
        "subject.json",
        &Subject {
            sha256: value.0,
            byte_count: value.1,
            original_path: Some(input.to_path_buf()),
        },
    )?;
    status(root, expected)
}

/// Read-only state. Changed context or input can never reuse an old approval.
pub fn status(root: &Path, expected: &ManualContext) -> Result<ManualStatus> {
    expected.validate()?;
    layout(root)?;
    let context: ManualContext = read(&root.join("context.json"))?;
    context.validate()?;
    let hash = canonical_sha256(expected)?;
    let make = |state, subject, reason: &str| ManualStatus {
        schema: "aniflow.manual-status/v1".into(),
        context_sha256: hash.clone(),
        subject_sha256: subject,
        state,
        reason: reason.into(),
        review_is_declared: true,
    };
    if context != *expected {
        return Ok(make(
            ManualState::Invalidated,
            None,
            "Source, timeline, recipe or configuration changed; start a new checkpoint",
        ));
    }
    if !exists(&root.join("subject.json"))? {
        return Ok(make(
            ManualState::AwaitingInput,
            None,
            "Awaiting an explicitly supplied artifact; a partial file is not an attached input",
        ));
    }
    let subject: Subject = read(&root.join("subject.json"))?;
    require_sha256(&subject.sha256, "manual subject")?;
    if context.role == ManualRole::TimedLyrics {
        let original = subject
            .original_path
            .as_ref()
            .ok_or_else(|| invalid("manual source locator missing"))?;
        let wanted = (subject.sha256.clone(), subject.byte_count);
        if subject.byte_count == 0
            || subject.byte_count > MAXIMUM_INPUT
            || identity(&root.join("artifact"), MAXIMUM_INPUT)
                .ok()
                .as_ref()
                != Some(&wanted)
            || identity(original, MAXIMUM_INPUT).ok().as_ref() != Some(&wanted)
        {
            return Ok(make(
                ManualState::Invalidated,
                Some(subject.sha256),
                "Manually supplied bytes changed; old review cannot authorize them",
            ));
        }
    } else if subject.byte_count != 0
        || subject.original_path.is_some()
        || exists(&root.join("artifact"))?
    {
        return Err(invalid(
            "prepared-frame review is a parent-verified identity, not a loose manual file",
        ));
    }
    if !exists(&root.join("review.json"))? {
        return Ok(make(
            ManualState::AwaitingReview,
            Some(subject.sha256),
            "Exact artifact attached; explicit operator review is required",
        ));
    }
    let review: ManualReview = read(&root.join("review.json"))?;
    validate_review(&review)?;
    if review.context_sha256 != hash || review.subject_sha256 != subject.sha256 {
        return Ok(make(
            ManualState::Invalidated,
            Some(subject.sha256),
            "Review names different source/timing/configuration or artifact bytes",
        ));
    }
    Ok(make(
        match review.decision {
            ReviewDecision::Approve => ManualState::Approved,
            ReviewDecision::Reject => ManualState::Rejected,
        },
        Some(subject.sha256),
        "Retained operator declaration; not native, legal, identity or publishing acceptance",
    ))
}
pub(super) fn validate_review(review: &ManualReview) -> Result<()> {
    require_sha256(&review.context_sha256, "review context")?;
    require_sha256(&review.subject_sha256, "review subject")?;
    if review.schema != "aniflow.manual-review/v1"
        || review.reviewer.trim().is_empty()
        || review.reviewer.len() > 256
        || review.reviewer.chars().any(char::is_control)
        || review.notes.len() > 4096
        || review
            .notes
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(invalid(
            "manual review requires a bounded explicit reviewer, decision and exact context/subject",
        ));
    }
    Ok(())
}

/// Record an explicit user-supplied decision once; no default approval exists.
pub fn review(
    root: &Path,
    context: &ManualContext,
    declaration: &ManualReview,
) -> Result<ManualStatus> {
    validate_review(declaration)?;
    layout(root)?;
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let current = status(root, context)?;
    if current.state != ManualState::AwaitingReview
        || declaration.context_sha256 != current.context_sha256
        || Some(declaration.subject_sha256.as_str()) != current.subject_sha256.as_deref()
    {
        return Err(invalid(
            "review must name the current awaiting-review context and exact subject",
        ));
    }
    write(root, "review.json", declaration)?;
    status(root, context)
}

pub(super) fn prepared_context(
    request: &super::MusicVideoRequest,
    intent: &str,
    frames: &super::FrameHandoff,
) -> Result<(ManualContext, String)> {
    let context = ManualContext {
        schema: "aniflow.manual-context/v1".into(),
        role: ManualRole::PreparedFrames,
        source_sha256: frames.source_sha256.clone(),
        timeline_sha256: frames.index_sha256.clone(),
        configuration_sha256: canonical_sha256(&request.configuration)?,
        recipe_intent_sha256: intent.into(),
    };
    let subject = canonical_sha256(
        &serde_json::json!({"frames":frames.directory,"index_sha256":frames.index_sha256,"ordered_frame_hashes":frames.frame_hashes}),
    )?;
    context.validate()?;
    Ok((context, subject))
}
pub(super) fn ensure_prepared(root: &Path, context: &ManualContext, subject: &str) -> Result<()> {
    if !exists(root)? {
        create(root, context)?;
    }
    layout(root)?;
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let saved: ManualContext = read(&root.join("context.json"))?;
    if saved != *context {
        return Err(invalid(
            "prepared-frame review context changed; preserve prior declarations and start a new recipe",
        ));
    }
    if !exists(&root.join("subject.json"))? {
        write(
            root,
            "subject.json",
            &Subject {
                sha256: subject.into(),
                byte_count: 0,
                original_path: None,
            },
        )?;
    }
    let value: Subject = read(&root.join("subject.json"))?;
    if value.sha256 != subject {
        return Err(invalid("prepared-frame review subject changed"));
    }
    Ok(())
}
pub(super) fn prepared_status(
    root: &Path,
    context: &ManualContext,
    subject: &str,
) -> Result<ManualStatus> {
    if !exists(root)? {
        return Ok(ManualStatus {
            schema: "aniflow.manual-status/v1".into(),
            context_sha256: canonical_sha256(context)?,
            subject_sha256: Some(subject.into()),
            state: ManualState::AwaitingReview,
            reason: "Awaiting an explicit prepared-frame review checkpoint".into(),
            review_is_declared: true,
        });
    }
    let mut current = status(root, context)?;
    if current.subject_sha256.as_deref() != Some(subject) {
        current.state = ManualState::Invalidated;
        current.reason =
            "Prepared frame set or index changed; prior review cannot authorize it".into();
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> ManualContext {
        ManualContext {
            schema: "aniflow.manual-context/v1".into(),
            role: ManualRole::TimedLyrics,
            source_sha256: "a".repeat(64),
            timeline_sha256: "b".repeat(64),
            configuration_sha256: "c".repeat(64),
            recipe_intent_sha256: "d".repeat(64),
        }
    }
    fn approval(status: &ManualStatus) -> ManualReview {
        ManualReview {
            schema: "aniflow.manual-review/v1".into(),
            context_sha256: status.context_sha256.clone(),
            subject_sha256: status.subject_sha256.clone().unwrap(),
            decision: ReviewDecision::Approve,
            reviewer: "synthetic reviewer".into(),
            notes: "Synthetic declaration only".into(),
        }
    }
    #[test]
    fn manual_input_review_and_mutated_source_have_distinct_truthful_states() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        let checkpoint = base.join("review");
        let source = base.join("manual.srt");
        let context = context();
        assert_eq!(
            create(&checkpoint, &context).unwrap().state,
            ManualState::AwaitingInput
        );
        fs::write(&source, b"1\n00:00:00,000 --> 00:00:01,000\nWords\n").unwrap();
        let pending = attach_file(
            &checkpoint,
            &context,
            &source,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(pending.state, ManualState::AwaitingReview);
        assert_eq!(
            review(&checkpoint, &context, &approval(&pending))
                .unwrap()
                .state,
            ManualState::Approved
        );
        fs::write(&source, b"changed words/timing").unwrap();
        assert_eq!(
            status(&checkpoint, &context).unwrap().state,
            ManualState::Invalidated
        );
        assert!(checkpoint.join("review.json").is_file());
    }
    #[test]
    fn changed_timeline_and_wrong_subject_cannot_reuse_approval() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        let checkpoint = base.join("review");
        let source = base.join("manual.srt");
        let context = context();
        create(&checkpoint, &context).unwrap();
        fs::write(&source, b"manual fixture").unwrap();
        let pending = attach_file(
            &checkpoint,
            &context,
            &source,
            &CancellationToken::default(),
        )
        .unwrap();
        let mut wrong = approval(&pending);
        wrong.subject_sha256 = "f".repeat(64);
        assert!(review(&checkpoint, &context, &wrong).is_err());
        let mut changed = context.clone();
        changed.timeline_sha256 = "e".repeat(64);
        assert_eq!(
            status(&checkpoint, &changed).unwrap().state,
            ManualState::Invalidated
        );
        assert!(!checkpoint.join("review.json").exists());
    }
    #[test]
    fn rejected_prepared_frames_stay_rejected_without_implicit_second_decision() {
        let root = tempfile::tempdir().unwrap();
        let checkpoint = root.path().canonicalize().unwrap().join("review");
        let mut context = context();
        context.role = ManualRole::PreparedFrames;
        ensure_prepared(&checkpoint, &context, &"e".repeat(64)).unwrap();
        let pending = status(&checkpoint, &context).unwrap();
        let mut declaration = approval(&pending);
        declaration.decision = ReviewDecision::Reject;
        assert_eq!(
            review(&checkpoint, &context, &declaration).unwrap().state,
            ManualState::Rejected
        );
        assert!(review(&checkpoint, &context, &approval(&pending)).is_err());
    }
}
