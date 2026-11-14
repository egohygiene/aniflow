//! Advice projected from the existing owner. Not a plan or execution token.
use super::{FinishingPhase, FinishingStatus, FinishingStepState, state};
use crate::{CancellationToken, PipelineV3RunState, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FinishingAction {
    StartNative {
        phase: FinishingPhase,
        plan_sha256: String,
    },
    InspectNativeRecovery {
        phase: FinishingPhase,
        native_state: PipelineV3RunState,
        manifest_sha256: String,
    },
    SupplyPreviewDecision {
        context_sha256: String,
        subject_sha256: String,
    },
    Rejected {
        context_sha256: String,
        subject_sha256: String,
    },
    ReviewFinalCandidate {},
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NextAction {
    pub schema: String,
    pub intent_sha256: String,
    pub action: FinishingAction,
    pub execution_authority: String,
    pub publication_authority: String,
}
fn terminal(complete: bool, all_complete: bool, has_completion: bool) -> Result<bool> {
    if complete != all_complete || complete != has_completion {
        return Err(state(
            "finishing status has contradictory completion evidence",
        ));
    }
    Ok(complete)
}
fn decision(
    phase: FinishingPhase,
    status: FinishingStepState,
    context: &str,
    subject: &str,
) -> Result<FinishingAction> {
    if phase != FinishingPhase::CaptionReview {
        return Err(state("review state cannot authorize a native media phase"));
    }
    crate::provider::require_sha256(context, "preview context")?;
    crate::provider::require_sha256(subject, "preview subject")?;
    match status {
        FinishingStepState::AwaitingReview => Ok(FinishingAction::SupplyPreviewDecision {
            context_sha256: context.into(),
            subject_sha256: subject.into(),
        }),
        FinishingStepState::Rejected => Ok(FinishingAction::Rejected {
            context_sha256: context.into(),
            subject_sha256: subject.into(),
        }),
        _ => Err(state(
            "caption decision requested outside a current preview review boundary",
        )),
    }
}
fn project(current: &FinishingStatus) -> Result<FinishingAction> {
    if current.steps.is_empty() || current.steps.len() > 4 {
        return Err(state(
            "finishing status must contain the fixed bounded phase set",
        ));
    }
    if terminal(
        current.complete,
        current
            .steps
            .iter()
            .all(|s| s.state == FinishingStepState::Complete),
        current.completion.is_some(),
    )? {
        return Ok(FinishingAction::ReviewFinalCandidate {});
    }
    let next = current
        .steps
        .iter()
        .find(|s| s.state != FinishingStepState::Complete)
        .ok_or_else(|| state("finishing next phase missing"))?;
    match next.state {
        FinishingStepState::AwaitingReview | FinishingStepState::Rejected => {
            let preview = current
                .preview
                .as_ref()
                .ok_or_else(|| state("review action requires current accepted preview evidence"))?;
            decision(
                next.phase,
                next.state,
                &preview.binding.context_sha256,
                &preview.binding.subject_sha256,
            )
        }
        FinishingStepState::Pending => {
            if next.phase == FinishingPhase::CaptionReview || next.run_directory.is_some() {
                return Err(state(
                    "pending finishing phase cannot bypass a review or existing native child",
                ));
            }
            let plan = next
                .plan
                .as_ref()
                .ok_or_else(|| state("next native phase lacks its accepted predecessor plan"))?;
            plan.validate()
                .map_err(|_| state("next native finishing plan is invalid"))?;
            Ok(FinishingAction::StartNative {
                phase: next.phase,
                plan_sha256: plan.plan_sha256.clone(),
            })
        }
        FinishingStepState::Incomplete => {
            let root = next
                .run_directory
                .as_ref()
                .ok_or_else(|| state("incomplete finishing phase lacks its native child"))?;
            let manifest = crate::status_v3(root)?;
            if next.native_manifest_sha256.as_ref() != Some(&manifest.manifest_sha256) {
                return Err(state(
                    "native finishing state changed during next-action observation",
                ));
            }
            Ok(FinishingAction::InspectNativeRecovery {
                phase: next.phase,
                native_state: manifest.payload.state,
                manifest_sha256: manifest.manifest_sha256,
            })
        }
        FinishingStepState::Complete => Err(state(
            "completed phase cannot be the next unfinished action",
        )),
    }
}
/// Re-observe the fixed finishing status before and after projecting advice.
/// Running/failed/interrupted children require native inspection/recovery;
/// this function neither retries them nor creates an approval or workspace.
pub fn observe(root: &Path, token: &CancellationToken) -> Result<NextAction> {
    let before = super::status(root, token)?;
    let action = project(&before)?;
    if super::status(root, token)? != before {
        return Err(state("finishing changed during next-action observation"));
    }
    if token.is_cancelled() {
        return Err(state("finishing next-action observation cancelled"));
    }
    Ok(NextAction {
        schema: "aniflow.finishing-next-action/v1".into(),
        intent_sha256: before.intent_sha256,
        action,
        execution_authority:
            "none; explicitly invoke the existing run/resume/review owner and its native guards"
                .into(),
        publication_authority: "none".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_requires_all_native_steps_and_final_evidence() {
        assert!(terminal(true, true, true).unwrap());
        assert!(!terminal(false, false, false).unwrap());
        for (a, b, c) in [
            (true, false, true),
            (true, true, false),
            (false, true, false),
            (false, false, true),
        ] {
            assert!(terminal(a, b, c).is_err());
        }
    }
    #[test]
    fn preview_decision_is_bound_to_exact_context_and_subject() {
        let action = decision(
            FinishingPhase::CaptionReview,
            FinishingStepState::AwaitingReview,
            &"a".repeat(64),
            &"b".repeat(64),
        )
        .unwrap();
        assert!(matches!(
            action,
            FinishingAction::SupplyPreviewDecision { .. }
        ));
        assert!(
            decision(
                FinishingPhase::CaptionReview,
                FinishingStepState::AwaitingReview,
                "bad",
                &"b".repeat(64)
            )
            .is_err()
        );
    }
    #[test]
    fn rejection_does_not_become_resume_or_an_approval() {
        let action = decision(
            FinishingPhase::CaptionReview,
            FinishingStepState::Rejected,
            &"a".repeat(64),
            &"b".repeat(64),
        )
        .unwrap();
        assert!(matches!(action, FinishingAction::Rejected { .. }));
        assert!(
            decision(
                FinishingPhase::CaptionDelivery,
                FinishingStepState::Rejected,
                &"a".repeat(64),
                &"b".repeat(64)
            )
            .is_err()
        );
    }
    #[test]
    fn ordinary_pending_work_cannot_request_or_fabricate_review() {
        assert!(
            decision(
                FinishingPhase::CaptionReview,
                FinishingStepState::Pending,
                &"a".repeat(64),
                &"b".repeat(64)
            )
            .is_err()
        );
    }
}
