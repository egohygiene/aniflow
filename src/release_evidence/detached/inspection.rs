//! Current copied-subject observation plus separate bounded OTS interpretation.
//! The original opaque receipt is immutable and retains its original authority.
use super::*;
use crate::release_evidence::ots::{self, ProofInspection, StructureState, SubjectBinding};
use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairInspection {
    pub schema: String,
    pub receipt_sha256: String,
    pub retained: ProofReceipt,
    pub proof: ProofInspection,
    pub byte_integrity: String,
    pub producer_acceptance: String,
    pub timestamp_state: String,
    pub ownership: String,
}
fn proof_bytes(path: &Path, token: &CancellationToken) -> Result<Vec<u8>> {
    let mut file = crate::audio_inspection::process::open_regular(path)
        .map_err(|_| invalid("cannot open retained OTS proof"))?;
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 65_536];
    loop {
        cancelled(token)?;
        let count = file
            .read(&mut buffer)
            .map_err(|_| invalid("cannot read retained OTS proof"))?;
        if count == 0 {
            return Ok(bytes);
        }
        if bytes
            .len()
            .checked_add(count)
            .is_none_or(|n| n > ots::MAXIMUM_PROOF_BYTES)
        {
            return Err(invalid("retained OTS proof exceeds 1 MiB"));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}
/// Rehash the exact retained subject and proof against an external receipt pin.
/// Return mismatched/unsupported interpretations explicitly without promoting
/// them to a successful binding or asserting chain-verifier/timestamp trust.
pub fn inspect(
    root: &Path,
    receipt_sha256: &str,
    token: &CancellationToken,
) -> Result<PairInspection> {
    cancelled(token)?;
    let retained = super::inspect(root, receipt_sha256, token)?;
    let subject = identity(&root.join("subject.bin"), MAX_SUBJECT)?;
    if subject != (retained.subject_sha256.clone(), retained.subject_bytes) {
        return Err(invalid(
            "copied proof subject changed before OTS interpretation",
        ));
    }
    let proof = ots::inspect(
        &proof_bytes(&root.join("proof.ots"), token)?,
        &subject.0,
        token,
    )?;
    if proof.proof_sha256 != retained.proof_sha256 || proof.proof_byte_count != retained.proof_bytes
    {
        return Err(invalid(
            "parsed OTS bytes differ from the exact retained candidate",
        ));
    }
    if super::inspect(root, receipt_sha256, token)? != retained
        || identity(&root.join("subject.bin"), MAX_SUBJECT)? != subject
    {
        return Err(invalid(
            "retained subject/proof receipt changed during OTS inspection",
        ));
    }
    cancelled(token)?;
    Ok(PairInspection {
        schema: "aniflow.retained-ots-inspection/v1".into(),
        receipt_sha256: receipt_sha256.into(),
        retained,
        proof,
        byte_integrity: "matches_external_receipt_and_retained_members".into(),
        producer_acceptance: "not_reobserved; archival_bytes_only".into(),
        timestamp_state: "not_verified".into(),
        ownership: "not_established".into(),
    })
}
impl PairInspection {
    /// Only the local subject/header/operation relationship, never trusted time.
    pub fn subject_matches(&self) -> bool {
        self.proof.structure == StructureState::Parsed
            && self.proof.subject_binding == SubjectBinding::MatchesExpectedSha256
    }
}
/// Refuse malformed, mismatched or unsupported OTS input before copying; then
/// use the existing exact-byte restoration owner and reobserve the new pair.
/// This creates no calendar/Bitcoin verification, proof upgrade or new receipt.
pub fn restore_bound(
    request: &restoration::RestoreRequest,
    root: &Path,
    token: &CancellationToken,
) -> Result<PairInspection> {
    request.validate()?;
    let before = inspect(&request.source_directory, &request.receipt_sha256, token)?;
    if !before.subject_matches() {
        return Err(invalid(
            "bound restore requires a supported OTS proof matching the actual retained subject; opaque restoration remains separate",
        ));
    }
    restoration::restore(request, root, token)?;
    let after = inspect(root, &request.receipt_sha256, token)?;
    if after != before
        || inspect(&request.source_directory, &request.receipt_sha256, token)? != before
    {
        return Err(invalid(
            "restored OTS relationship differs from its exact source; preserve the retained workspace",
        ));
    }
    Ok(after)
}

#[cfg(test)]
mod tests;
