//! Explicit review delivery over the fixed recipe's existing accepted-frame gate.
use super::manual::{ManualContext, ManualReview, ManualState, ManualStatus};
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedFrameReview {
    pub schema: String,
    pub checkpoint: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub frame_count: u64,
    pub frames_sha256: String,
    pub index_sha256: String,
    pub context: ManualContext,
    pub review: ManualStatus,
}
pub(super) fn observed(
    root: &Path,
    frames: &FrameHandoff,
    context: ManualContext,
    review: ManualStatus,
) -> PreparedFrameReview {
    PreparedFrameReview {
        schema: "aniflow.prepared-frame-review/v1".into(),
        checkpoint: root.join("review"),
        frames_directory: frames.frames.clone(),
        frame_index: frames.index.clone(),
        frame_count: frames.directory.file_count,
        frames_sha256: frames.directory.content_sha256.clone(),
        index_sha256: frames.index_sha256.clone(),
        context,
        review,
    }
}
fn capture(root: &Path) -> Result<PreparedFrameReview> {
    let (request, intent) = resolved_request(root)?;
    if !request.configuration.review_prepared_frames {
        return Err(state(
            "prepared-frame review is not selected in this immutable recipe",
        ));
    }
    let token = CancellationToken::default();
    let initial = plan(&request, &token)?;
    observe_tail_details(&request,root,&intent,&initial,&token)?.1
        .ok_or_else(||state("prepared-frame review awaits currently accepted predecessor frames and any interpolation carrier; inspect recipe status"))
}
/// Reobserve exact prepared/interpolated frames without writing or launching tools.
/// Private locators identify what the operator must actually inspect.
pub fn status(run_directory: impl AsRef<Path>) -> Result<PreparedFrameReview> {
    capture(&workspace::root(run_directory.as_ref())?)
}
/// Retain one supplied exact decision. Does not run/resume assembly, create a
/// declaration, authenticate a reviewer or grant publishing authority.
pub fn record(
    run_directory: impl AsRef<Path>,
    declaration: &ManualReview,
) -> Result<PreparedFrameReview> {
    manual::validate_review(declaration)?;
    let root = workspace::root(run_directory.as_ref())?;
    // Validate the fixed intent/layout before reserving a parent writer.
    resolved_request(&root)?;
    let _parent = crate::cache_v3::ExclusiveWriter::acquire(&root, "writer.lock")?;
    let mut writers = Vec::new();
    for name in [
        "extract_frames",
        "prepare_frames",
        "upscale",
        "interpolate",
        "assembly",
    ] {
        let child = root.join("children").join(name);
        if workspace::exists(&child)? {
            writers.push(crate::cache_v3::ExclusiveWriter::acquire(
                &child,
                "writer.lock",
            )?);
        }
    }
    if workspace::exists(&workspace::child(&root, MusicVideoPhase::Assembly))? {
        return Err(state(
            "assembly is already initialized; preserve its review and accepted work, never replace a prior decision",
        ));
    }
    let before = capture(&root)?;
    if before.review.state != ManualState::AwaitingReview
        || before.review.context_sha256 != declaration.context_sha256
        || before.review.subject_sha256.as_deref() != Some(declaration.subject_sha256.as_str())
    {
        return Err(state(
            "decision must name the current awaiting prepared-frame review and exact subject",
        ));
    }
    manual::ensure_prepared(
        &before.checkpoint,
        &before.context,
        &declaration.subject_sha256,
    )?;
    manual::review(&before.checkpoint, &before.context, declaration)?;
    let after = capture(&root)?;
    if before.context != after.context
        || before.review.subject_sha256 != after.review.subject_sha256
        || matches!(
            after.review.state,
            ManualState::Invalidated | ManualState::AwaitingInput | ManualState::AwaitingReview
        )
    {
        return Err(state(
            "prepared frames or recipe changed while retaining the supplied declaration; preserve it and inspect status",
        ));
    }
    drop(writers);
    Ok(after)
}
