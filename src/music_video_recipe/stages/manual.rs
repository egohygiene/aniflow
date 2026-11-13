//! Explicit manual delivery, delegated to existing domain writers and reviews.
use super::*;
use crate::music_video_recipe::manual::ManualReview;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    ImportLyrics {
        input: PathBuf,
        input_sha256: String,
        operator: String,
        declared_origin: String,
    },
    ExportLyrics {},
    RecordReview {
        scope: ReviewScope,
        declaration: ManualReview,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub schema: String,
    pub expected_intent_sha256: String,
    pub expected_context_sha256: String,
    pub operation: Operation,
}
impl Change {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 32_768 {
            return Err(invalid("manual stage change exceeds 32 KiB"));
        }
        let value: Self = decode_json(bytes, "supplied manual stage change")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        require_sha256(
            &self.expected_intent_sha256,
            "expected release-stage intent",
        )?;
        require_sha256(
            &self.expected_context_sha256,
            "expected release-stage context",
        )?;
        if self.schema != "aniflow.release-stage-manual-change/v1" {
            return Err(invalid("closed supplied manual stage change required"));
        }
        match &self.operation {
            Operation::ImportLyrics {
                input,
                input_sha256,
                operator,
                declared_origin,
            } => {
                require_sha256(input_sha256, "explicit manual lyric input")?;
                if !input.is_absolute()
                    || input.components().any(|c| {
                        matches!(
                            c,
                            std::path::Component::ParentDir | std::path::Component::CurDir
                        )
                    })
                    || operator.trim().is_empty()
                    || operator.len() > 256
                    || operator.chars().any(char::is_control)
                    || declared_origin.trim().is_empty()
                    || declared_origin.len() > 1024
                    || declared_origin.chars().any(char::is_control)
                {
                    return Err(invalid(
                        "manual lyric import requires an exact external path, digest and bounded operator-declared origin",
                    ));
                }
            }
            Operation::RecordReview { declaration, .. } => declaration.validate()?,
            Operation::ExportLyrics {} => {}
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: String,
    pub change_sha256: String,
    pub applied_change: Change,
    pub before_context_sha256: String,
    pub after: Status,
    pub origin_authority: String,
    pub provider_execution_observed: bool,
    pub publication_authority: String,
}
fn admission<'a>(status: &'a Status, change: &Change) -> Result<&'a Step> {
    if status.intent_sha256 != change.expected_intent_sha256
        || status.context_sha256 != change.expected_context_sha256
    {
        return Err(invalid(
            "manual stage change is stale or names different selected settings",
        ));
    }
    let next = status
        .next_stage
        .ok_or_else(|| invalid("selected release journey has no pending manual work"))?;
    let step = status
        .steps
        .iter()
        .find(|s| s.stage == next)
        .ok_or_else(|| invalid("next selected stage missing"))?;
    let allowed = match &change.operation {
        Operation::ImportLyrics { .. } => {
            step.stage == Stage::TimedLyrics
                && step.state == State::AwaitingInput
                && step.handoff.action == Action::SupplyTimedLyrics
        }
        Operation::ExportLyrics {} => {
            step.stage == Stage::TimedLyrics
                && step.state == State::AwaitingInput
                && step.handoff.action == Action::ExportReviewedLyrics
        }
        Operation::RecordReview { scope, declaration } => {
            step.state == State::AwaitingReview
                && step.handoff.reviews.iter().any(|r| {
                    r.scope == *scope
                        && r.state == State::AwaitingReview
                        && r.context_sha256 == declaration.context_sha256
                        && r.subject_sha256.as_deref() == Some(declaration.subject_sha256.as_str())
                })
        }
    };
    if !allowed {
        return Err(invalid(
            "manual action does not match the current exact awaiting-input/review handoff",
        ));
    }
    Ok(step)
}
/// Caller supplies every action/decision. Existing writers retain snapshots,
/// validation and declarations; this API creates no new workspace or ledger.
/// The returned private receipt must be retained explicitly by its caller.
pub fn apply(request: &Request, change: &Change, token: &CancellationToken) -> Result<Receipt> {
    change.validate()?;
    cancelled(token)?;
    let before = observe(request, token)?;
    let selected = admission(&before, change)?;
    let intent = request
        .stages
        .iter()
        .find(|s| s.stage == selected.stage)
        .ok_or_else(|| invalid("selected stage intent missing"))?;
    cancelled(token)?;
    match (&change.operation, &intent.selection) {
        (
            Operation::ImportLyrics {
                input,
                input_sha256,
                ..
            },
            Some(Selection::Lyrics {
                request,
                run_directory,
                ..
            }),
        ) => {
            if crate::frame_io::identity(input, crate::timed_text::MAX_TEXT_BYTES as u64)?.0
                != *input_sha256
            {
                return Err(invalid(
                    "manual lyric bytes changed from the supplied digest",
                ));
            }
            if run_directory
                .try_exists()
                .map_err(|_| invalid("cannot inspect manual lyrics root"))?
            {
                lyrics::import(request, run_directory, input, token)?;
            } else {
                lyrics::initialize(request, run_directory, Some(input), token)?;
            }
            if crate::frame_io::identity(input, crate::timed_text::MAX_TEXT_BYTES as u64)?.0
                != *input_sha256
            {
                return Err(invalid(
                    "manual lyrics changed during import; preserve the owner's partial state",
                ));
            }
        }
        (
            Operation::ExportLyrics {},
            Some(Selection::Lyrics {
                request,
                run_directory,
                delivery_directory: Some(output),
            }),
        ) => {
            lyrics::export(request, run_directory, output)?;
        }
        (
            Operation::RecordReview { scope, declaration },
            Some(Selection::Lyrics {
                request,
                run_directory,
                ..
            }),
        ) => {
            let scope = match scope {
                ReviewScope::Words => lyrics::LyricsReviewScope::Words,
                ReviewScope::Timing => lyrics::LyricsReviewScope::Timing,
                _ => return Err(invalid("review scope belongs to another existing owner")),
            };
            lyrics::review(request, run_directory, scope, declaration)?;
        }
        (
            Operation::RecordReview {
                scope: ReviewScope::Caption,
                declaration,
            },
            Some(Selection::Finishing {
                run_directory: Some(root),
                ..
            }),
        ) => {
            finishing::record_review(root, declaration, token)?;
        }
        (
            Operation::RecordReview {
                scope: ReviewScope::FinalVideo,
                declaration,
            },
            Some(Selection::Publishing {
                run_directory: Some(root),
                ..
            }),
        ) => {
            publishing::record_review(root, declaration, token)?;
        }
        _ => {
            return Err(invalid(
                "manual action requires a current concrete owner/root; no fallback",
            ));
        }
    }
    cancelled(token)?;
    let after = observe(request, token)?;
    Ok(Receipt {
        schema: "aniflow.release-stage-manual-receipt/v1".into(),
        change_sha256: canonical_sha256(change)?,
        applied_change: change.clone(),
        before_context_sha256: before.context_sha256,
        after,
        origin_authority: "operator_declared; retained input bytes and existing checks only".into(),
        provider_execution_observed: false,
        publication_authority: "none".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn change() -> Change {
        Change {
            schema: "aniflow.release-stage-manual-change/v1".into(),
            expected_intent_sha256: "a".repeat(64),
            expected_context_sha256: "b".repeat(64),
            operation: Operation::ImportLyrics {
                input: "/synthetic/source.srt".into(),
                input_sha256: "c".repeat(64),
                operator: "synthetic operator declaration".into(),
                declared_origin: "manually authored interval lyrics".into(),
            },
        }
    }
    #[test]
    fn manual_import_requires_exact_bytes_and_declared_origin() {
        let mut value = change();
        assert!(value.validate().is_ok());
        if let Operation::ImportLyrics {
            declared_origin, ..
        } = &mut value.operation
        {
            declared_origin.clear();
        }
        assert!(value.validate().is_err());
        value = change();
        if let Operation::ImportLyrics { input_sha256, .. } = &mut value.operation {
            input_sha256.clear();
        }
        assert!(value.validate().is_err());
    }
    #[test]
    fn no_default_approval_or_command_operation_exists() {
        let mut value = serde_json::to_value(change()).unwrap();
        value["operation"] = serde_json::json!({"kind":"record_review","scope":"final_video"});
        assert!(Change::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
        value["operation"] = serde_json::json!({"kind":"run","command":"auto-approve"});
        assert!(Change::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn unknown_authority_and_path_traversal_refuse() {
        let mut value = serde_json::to_value(change()).unwrap();
        value["approval_authority"] = serde_json::json!(true);
        assert!(Change::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
        let mut value = change();
        if let Operation::ImportLyrics { input, .. } = &mut value.operation {
            *input = "/private/../other.srt".into();
        }
        assert!(value.validate().is_err());
    }
    #[test]
    fn cancelled_manual_action_never_reaches_owner_writers() {
        let request = super::super::tests::request();
        let token = CancellationToken::new();
        token.cancel();
        assert!(apply(&request, &change(), &token).is_err());
    }
}
