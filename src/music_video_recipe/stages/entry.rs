//! Read-only source/indexed entry composition before an accepted master exists.
//! Existing recipe/review/final-video owners retain all acceptance and writes.
use super::*;
use crate::music_video_recipe::{
    self as recipe, MusicVideoEntryKind, MusicVideoPhase, MusicVideoPhaseState,
    SourceMusicVideoPhase,
    manual::{ManualReview, ManualState},
    prepared_review,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub recipe_run_directory: PathBuf,
    pub expected_recipe_intent_sha256: String,
    pub artifact_id: String,
    pub release_request: Option<super::Request>,
}
impl Request {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 524_288 {
            return Err(invalid("source-entry journey request exceeds512KiB"));
        }
        let value: Self = decode_json(bytes, "source-entry journey request")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        require_sha256(
            &self.expected_recipe_intent_sha256,
            "expected existing recipe intent",
        )?;
        if self.schema != "aniflow.release-stages-entry-request/v1"
            || !destination::identifier(&self.artifact_id)
            || !self.recipe_run_directory.is_absolute()
            || self.recipe_run_directory.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(invalid(
                "explicit existing source/indexed recipe, intent and output selection required",
            ));
        }
        if let Some(release) = &self.release_request {
            release.validate()?;
            match &release.source {
                MediaInput::AcceptedVideo {
                    run_directory,
                    artifact_id,
                } if *run_directory == self.recipe_run_directory
                    && *artifact_id == self.artifact_id => {}
                _ => {
                    return Err(invalid(
                        "release tail must select this exact recipe/output, never another master",
                    ));
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub phase: SourceMusicVideoPhase,
    pub state: MusicVideoPhaseState,
    pub run_directory: Option<PathBuf>,
    pub plan_sha256: Option<String>,
    pub native_manifest_sha256: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    Pending,
    Ready,
    Running,
    AwaitingInput,
    AwaitingReview,
    Rejected,
    Invalidated,
    Incomplete,
    Failed,
    Cancelled,
    MasterAccepted,
    ReleasePending,
    ReleaseComplete,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    ResumeExistingRecipe,
    InspectNativeRecovery,
    SupplyPreparedFrameDecision,
    StartNewContext,
    ConfigureReleaseTail,
    InspectReleaseTail,
    None,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub schema: String,
    pub context_sha256: String,
    pub entry_kind: MusicVideoEntryKind,
    pub recipe_intent_sha256: String,
    pub configuration_sha256: String,
    pub initial_inputs: Vec<crate::PipelineInputIdentity>,
    pub phases: Vec<Phase>,
    pub state: EntryState,
    pub next_action: NextAction,
    pub prepared_frames: Option<prepared_review::PreparedFrameReview>,
    pub recipe_complete: bool,
    pub master: Option<crate::final_video::FinalVideoHandoff>,
    pub release: Option<super::Status>,
    pub release_observed: bool,
    pub native_execution_authority: String,
    pub creative_approval: String,
    pub publication_authority: String,
}
fn pending(phases: &[Phase], review: Option<ManualState>) -> Result<(EntryState, NextAction)> {
    if let Some(state) = review {
        match state {
            ManualState::AwaitingReview => {
                return Ok((
                    EntryState::AwaitingReview,
                    NextAction::SupplyPreparedFrameDecision,
                ));
            }
            ManualState::AwaitingInput => {
                return Ok((EntryState::AwaitingInput, NextAction::InspectNativeRecovery));
            }
            ManualState::Rejected => {
                return Ok((EntryState::Rejected, NextAction::StartNewContext));
            }
            ManualState::Invalidated => {
                return Ok((EntryState::Invalidated, NextAction::StartNewContext));
            }
            ManualState::Approved => {}
        }
    }
    let Some(phase) = phases.iter().find(|p| {
        !matches!(
            p.state,
            MusicVideoPhaseState::Complete
                | MusicVideoPhaseState::Disabled
                | MusicVideoPhaseState::AlreadyTarget
                | MusicVideoPhaseState::AboveTarget
        )
    }) else {
        return Ok((EntryState::Incomplete, NextAction::InspectNativeRecovery));
    };
    Ok(match phase.state {
        MusicVideoPhaseState::Pending => (EntryState::Pending, NextAction::ResumeExistingRecipe),
        MusicVideoPhaseState::Ready => (EntryState::Ready, NextAction::ResumeExistingRecipe),
        MusicVideoPhaseState::Running => (EntryState::Running, NextAction::InspectNativeRecovery),
        MusicVideoPhaseState::AwaitingInput => {
            (EntryState::AwaitingInput, NextAction::InspectNativeRecovery)
        }
        MusicVideoPhaseState::AwaitingReview => (
            EntryState::AwaitingReview,
            NextAction::InspectNativeRecovery,
        ),
        MusicVideoPhaseState::IncompleteInitialization => {
            (EntryState::Incomplete, NextAction::InspectNativeRecovery)
        }
        MusicVideoPhaseState::Failed => (EntryState::Failed, NextAction::InspectNativeRecovery),
        MusicVideoPhaseState::Cancelled => {
            (EntryState::Cancelled, NextAction::InspectNativeRecovery)
        }
        _ => return Err(invalid("resolved phase cannot be next incomplete phase")),
    })
}
fn indexed(phase: MusicVideoPhase) -> SourceMusicVideoPhase {
    match phase {
        MusicVideoPhase::PrepareFrames => SourceMusicVideoPhase::PrepareFrames,
        MusicVideoPhase::Upscale => SourceMusicVideoPhase::Upscale,
        MusicVideoPhase::Interpolate => SourceMusicVideoPhase::Interpolate,
        MusicVideoPhase::Assembly => SourceMusicVideoPhase::Assembly,
    }
}
fn project(request: &Request, token: &CancellationToken) -> Result<Status> {
    request.validate()?;
    cancelled(token)?;
    let root = recipe::workspace::root(&request.recipe_run_directory)?;
    let entry_kind = recipe::entry_kind(&root)?;
    let (intent, configuration_sha256, initial_inputs, phases, recipe_complete, review_selected) =
        match entry_kind {
            MusicVideoEntryKind::Indexed => {
                let saved = recipe::workspace::read_intent(&root)?;
                if saved.intent_sha256 != request.expected_recipe_intent_sha256 {
                    return Err(invalid("indexed recipe differs from the selected intent"));
                }
                let current = recipe::status(&root)?;
                if current.intent_sha256 != saved.intent_sha256 {
                    return Err(invalid("indexed recipe intent changed during observation"));
                }
                let phases = current
                    .phases
                    .into_iter()
                    .map(|p| Phase {
                        phase: indexed(p.phase),
                        state: p.state,
                        run_directory: p.run_directory,
                        plan_sha256: p.plan_sha256,
                        native_manifest_sha256: p.manifest.map(|m| m.manifest_sha256),
                    })
                    .collect::<Vec<_>>();
                (
                    saved.intent_sha256,
                    canonical_sha256(&saved.payload.configuration)?,
                    saved.payload.initial_inputs,
                    phases,
                    current.complete,
                    saved.payload.configuration.review_prepared_frames,
                )
            }
            MusicVideoEntryKind::Source => {
                let saved = recipe::workspace::read_source_intent(&root)?;
                if saved.intent_sha256 != request.expected_recipe_intent_sha256 {
                    return Err(invalid("source recipe differs from the selected intent"));
                }
                let current = recipe::status_source(&root)?;
                if current.intent_sha256 != saved.intent_sha256 {
                    return Err(invalid("source recipe intent changed during observation"));
                }
                let phases = current
                    .phases
                    .into_iter()
                    .map(|p| Phase {
                        phase: p.phase,
                        state: p.state,
                        run_directory: p.run_directory,
                        plan_sha256: p.plan_sha256,
                        native_manifest_sha256: p.manifest.map(|m| m.manifest_sha256),
                    })
                    .collect::<Vec<_>>();
                (
                    saved.intent_sha256,
                    canonical_sha256(&saved.payload.configuration)?,
                    saved.payload.initial_inputs,
                    phases,
                    current.complete,
                    saved.payload.configuration.recipe.review_prepared_frames,
                )
            }
        };
    if intent != request.expected_recipe_intent_sha256 {
        return Err(invalid(
            "source-entry journey names different immutable recipe settings/source",
        ));
    }
    if phases.is_empty()
        || phases.len() > 5
        || phases
            .last()
            .is_none_or(|p| p.phase != SourceMusicVideoPhase::Assembly)
    {
        return Err(invalid(
            "existing recipe returned an inconsistent fixed phase set",
        ));
    }
    cancelled(token)?;
    // A ready assembly with no plan can still await accepted interpolation.
    // Do not manufacture a review subject until its actual owner can bind one.
    let ready_for_review = review_selected
        && phases.iter().any(|p| {
            p.phase == SourceMusicVideoPhase::Assembly
                && (p.state == MusicVideoPhaseState::AwaitingReview || p.plan_sha256.is_some())
        });
    let prepared_frames = if ready_for_review {
        Some(prepared_review::status(&root)?)
    } else {
        None
    };
    let mut master = None;
    let mut release = None;
    let (state, next_action) = if recipe_complete {
        if phases
            .last()
            .is_none_or(|p| p.state != MusicVideoPhaseState::Complete)
        {
            return Err(invalid("recipe completion lacks accepted assembly"));
        }
        let current = crate::final_video::observe(&root)?;
        let selected = current
            .artifacts
            .iter()
            .filter(|a| a.artifact_id == request.artifact_id)
            .collect::<Vec<_>>();
        if selected.len() != 1 {
            return Err(invalid("select one exact accepted master output"));
        }
        let state = if let Some(tail) = &request.release_request {
            if tail.source_sha256 != selected[0].sha256 {
                return Err(invalid(
                    "release tail does not name the actual current assembled bytes",
                ));
            }
            let observed = super::observe(tail, token)?;
            let complete = observed.complete;
            release = Some(observed);
            if complete {
                (EntryState::ReleaseComplete, NextAction::None)
            } else {
                (EntryState::ReleasePending, NextAction::InspectReleaseTail)
            }
        } else {
            (EntryState::MasterAccepted, NextAction::ConfigureReleaseTail)
        };
        master = Some(current);
        state
    } else {
        pending(&phases, prepared_frames.as_ref().map(|p| p.review.state))?
    };
    let mut output = Status {
        schema: "aniflow.release-stages-entry-status/v1".into(),
        context_sha256: String::new(),
        entry_kind,
        recipe_intent_sha256: intent,
        configuration_sha256,
        initial_inputs,
        phases,
        state,
        next_action,
        prepared_frames,
        recipe_complete,
        master,
        release_observed: release.is_some(),
        release,
        native_execution_authority: "none; use existing recipe run/resume explicitly".into(),
        creative_approval: "only retained exact operator declarations; no new creative approval"
            .into(),
        publication_authority: "none".into(),
    };
    output.context_sha256 = canonical_sha256(&(request, &output))?;
    local::bytes(&output)?;
    Ok(output)
}
/// Reobserve current source/indexed owner and all reached tail owners, without
/// running extraction/preparation/assembly or reading an unbound release tail.
pub fn observe(request: &Request, token: &CancellationToken) -> Result<Status> {
    let first = project(request, token)?;
    let second = project(request, token)?;
    if first != second {
        return Err(invalid(
            "source-entry recipe/review/acceptance changed during observation",
        ));
    }
    Ok(first)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub schema: String,
    pub expected_context_sha256: String,
    pub declaration: ManualReview,
}
impl Decision {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 {
            return Err(invalid("source-entry decision exceeds64KiB"));
        }
        let value: Self = decode_json(bytes, "supplied source-entry decision")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        require_sha256(
            &self.expected_context_sha256,
            "expected source-entry review context",
        )?;
        if self.schema != "aniflow.release-stages-entry-decision/v1" {
            return Err(invalid("closed supplied source-entry decision required"));
        }
        self.declaration.validate()
    }
}
fn decision_matches(status: &Status, decision: &Decision) -> Result<()> {
    if status.context_sha256 != decision.expected_context_sha256
        || status.state != EntryState::AwaitingReview
        || status.next_action != NextAction::SupplyPreparedFrameDecision
    {
        return Err(invalid(
            "source-entry decision is stale or not at the current prepared-frame review",
        ));
    }
    let current = status
        .prepared_frames
        .as_ref()
        .ok_or_else(|| invalid("current native frame review subject missing"))?;
    if current.review.state != ManualState::AwaitingReview
        || current.review.context_sha256 != decision.declaration.context_sha256
        || current.review.subject_sha256.as_deref()
            != Some(decision.declaration.subject_sha256.as_str())
    {
        return Err(invalid(
            "supplied source-entry decision names a different exact native frame set",
        ));
    }
    Ok(())
}
/// Retain only a fully supplied exact declaration through the existing review
/// owner. Never start/resume assembly, replace a prior decision or auto-approve.
pub fn record(request: &Request, decision: &Decision, token: &CancellationToken) -> Result<Status> {
    decision.validate()?;
    let before = observe(request, token)?;
    decision_matches(&before, decision)?;
    cancelled(token)?;
    prepared_review::record(&request.recipe_run_directory, &decision.declaration)?;
    cancelled(token)?;
    observe(request, token)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn phase(state: MusicVideoPhaseState) -> Phase {
        Phase {
            phase: SourceMusicVideoPhase::Assembly,
            state,
            run_directory: None,
            plan_sha256: None,
            native_manifest_sha256: None,
        }
    }
    #[test]
    fn prepared_frame_rejection_and_invalidation_never_become_resume_or_success() {
        assert_eq!(
            pending(
                &[phase(MusicVideoPhaseState::AwaitingReview)],
                Some(ManualState::Rejected)
            )
            .unwrap(),
            (EntryState::Rejected, NextAction::StartNewContext)
        );
        assert_eq!(
            pending(
                &[phase(MusicVideoPhaseState::AwaitingReview)],
                Some(ManualState::Invalidated)
            )
            .unwrap(),
            (EntryState::Invalidated, NextAction::StartNewContext)
        );
    }
    #[test]
    fn interrupted_or_running_native_work_requests_inspection_not_replay() {
        for state in [
            MusicVideoPhaseState::Failed,
            MusicVideoPhaseState::Cancelled,
            MusicVideoPhaseState::Running,
            MusicVideoPhaseState::IncompleteInitialization,
        ] {
            assert_eq!(
                pending(&[phase(state)], None).unwrap().1,
                NextAction::InspectNativeRecovery
            );
        }
    }
    #[test]
    fn resolved_native_phases_do_not_manufacture_parent_completion() {
        assert_eq!(
            pending(&[phase(MusicVideoPhaseState::Complete)], None).unwrap(),
            (EntryState::Incomplete, NextAction::InspectNativeRecovery)
        );
        assert_eq!(
            pending(
                &[phase(MusicVideoPhaseState::Ready)],
                Some(ManualState::Approved)
            )
            .unwrap(),
            (EntryState::Ready, NextAction::ResumeExistingRecipe)
        );
    }
    #[test]
    fn unknown_source_stage_authority_and_default_approval_are_not_accepted() {
        let raw = serde_json::json!({"schema":"aniflow.release-stages-entry-request/v1","recipe_run_directory":"/synthetic/absent","expected_recipe_intent_sha256":"a".repeat(64),"artifact_id":"master","release_request":null,"execute":true});
        assert!(Request::from_json_slice(&serde_json::to_vec(&raw).unwrap()).is_err());
        assert!(Decision::from_json_slice(br#"{"schema":"aniflow.release-stages-entry-decision/v1","expected_context_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#).is_err());
    }
    #[test]
    fn absent_native_source_or_cancellation_never_creates_a_master() {
        let request = Request {
            schema: "aniflow.release-stages-entry-request/v1".into(),
            recipe_run_directory: "/synthetic/absent".into(),
            expected_recipe_intent_sha256: "a".repeat(64),
            artifact_id: "master".into(),
            release_request: None,
        };
        assert!(observe(&request, &CancellationToken::new()).is_err());
        let token = CancellationToken::new();
        token.cancel();
        assert!(observe(&request, &token).is_err());
    }
}
