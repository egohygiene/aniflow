//! Shared fixed final review/package/retention inspection, with no runtime.
use super::*;
pub(super) struct Tail {
    pub steps: Vec<PublishingStep>,
    pub posting_plan: Option<posting::PostingPlan>,
    pub review: Option<ManualReview>,
    pub package: Option<posting::PostingReceipt>,
    pub retention: Option<RetentionHandoff>,
}
pub(super) fn retention(
    root: &Path,
    package: &PublishingPackage,
    settings: &PublishingRetention,
    token: &CancellationToken,
) -> Result<RetentionHandoff> {
    use crate::release_evidence::{
        EvidenceRequest, PostingMember, SubjectInput, SubjectRole, SubjectSelection,
    };
    let directory = package_root(root);
    let request = EvidenceRequest {
        schema: "aniflow.release-evidence-request/v1".into(),
        release_id: package.public_metadata.release_id.clone(),
        version: package.public_metadata.version.clone(),
        subjects: vec![
            SubjectSelection {
                id: "final_media".into(),
                role: SubjectRole::FinalDelivery,
                visibility: posting::MemberVisibility::Public,
                parents: vec![],
                input: SubjectInput::PostingMember {
                    directory: directory.clone(),
                    member: PostingMember::Media,
                },
            },
            SubjectSelection {
                id: "public_release_metadata".into(),
                role: SubjectRole::Sidecar,
                visibility: posting::MemberVisibility::Public,
                parents: vec!["final_media".into()],
                input: SubjectInput::PostingMember {
                    directory,
                    member: PostingMember::PublicMetadata,
                },
            },
        ],
        maximum_bundle_bytes: settings.maximum_bundle_bytes,
        minimum_free_bytes: settings.minimum_free_bytes,
    };
    let plan = crate::release_evidence::plan(&request, token)?;
    Ok(RetentionHandoff {
        schema: "aniflow.publishing-retention-handoff/v1".into(),
        request,
        plan,
        sealing_state: "not_performed; explicit downstream exact-byte retention remains separate"
            .into(),
        publication_authority: "none".into(),
    })
}
pub(super) fn inspect(
    root: &Path,
    package_settings: &PublishingPackage,
    retention_settings: Option<&PublishingRetention>,
    preflight: Option<PreflightReport>,
    token: &CancellationToken,
) -> Result<Tail> {
    let mut steps = Vec::new();
    let preflight_state = preflight.as_ref().map(|p| p.payload.disposition);
    let posting_plan = if preflight_state == Some(PreflightDisposition::MatchesSuppliedProfile) {
        Some(posting::plan(
            &posting_request_for(package_settings, preflight.clone().unwrap()),
            token,
        )?)
    } else {
        None
    };
    let review_path = root.join("posting-review.json");
    let mut review = None;
    if exists(&review_path)? {
        let value: ManualReview = local::read_bounded(&review_path, 65_536)?;
        require_review(
            posting_plan
                .as_ref()
                .ok_or_else(|| state("publishing decision exists before matching preflight"))?,
            &value,
        )?;
        review = Some(value);
    }
    let review_state = match review.as_ref().map(|r| r.decision) {
        Some(ReviewDecision::Approve) => PublishingStepState::Complete,
        Some(ReviewDecision::Reject) => PublishingStepState::Rejected,
        None if posting_plan.is_some() => PublishingStepState::AwaitingReview,
        None => PublishingStepState::Pending,
    };
    steps.push(step(PublishingPhase::FinalReview, review_state, None, None));
    let package_path = package_root(root);
    let mut package = None;
    if exists(&package_path)? {
        if review_state != PublishingStepState::Complete {
            return Err(state(
                "posting package exists before its exact supplied approval",
            ));
        }
        let current = posting::verify(&package_path, token)?;
        let expected = posting_plan.as_ref().unwrap();
        if current.package_id != expected.request_sha256
            || current.review_sha256 != canonical_sha256(review.as_ref().unwrap())?
            || current.preflight_sha256 != preflight.as_ref().unwrap().preflight_sha256
            || current.review_context_sha256 != expected.review_context_sha256
            || current.review_subject_sha256 != expected.review_subject_sha256
        {
            return Err(state(
                "publishing package differs from exact current request/decision/subject",
            ));
        }
        package = Some(current);
    }
    steps.push(step(
        PublishingPhase::ManualPackage,
        if package.is_some() {
            PublishingStepState::Complete
        } else {
            PublishingStepState::Pending
        },
        package.as_ref().map(|_| package_path),
        None,
    ));
    let mut handoff = None;
    if retention_settings.is_some() {
        let path = root.join("retention-handoff.json");
        if exists(&path)? {
            if package.is_none() {
                return Err(state(
                    "retention handoff exists before the exact accepted local posting package",
                ));
            }
            let saved: RetentionHandoff = local::read(&path)?;
            if saved != retention(root, package_settings, retention_settings.unwrap(), token)? {
                return Err(state(
                    "retention handoff no longer names exact current package bytes/origin",
                ));
            }
            handoff = Some(saved);
        }
        steps.push(step(
            PublishingPhase::RetentionHandoff,
            if handoff.is_some() {
                PublishingStepState::Complete
            } else {
                PublishingStepState::Pending
            },
            None,
            None,
        ));
    }

    Ok(Tail {
        steps,
        posting_plan,
        review,
        package,
        retention: handoff,
    })
}
