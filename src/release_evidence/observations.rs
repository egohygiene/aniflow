use super::*;
const MAXIMUM_OBSERVATIONS: u32 = 256;
const OBSERVATION_BYTES: u64 = 16_384;
const CLOCK: &str = "local_wall_clock; not trusted timestamp";
pub(super) fn chain(
    root: &Path,
    verification: &ByteVerification,
    token: &CancellationToken,
) -> Result<(u32, Option<String>)> {
    let directory = root.join("observations");
    ensure_canonical_directory(&directory)?;
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&directory)
        .map_err(|_| invalid("cannot inspect verification observations"))?
    {
        cancelled(token)?;
        if names.len() >= MAXIMUM_OBSERVATIONS as usize {
            return Err(invalid("verification observation count exceeds 256"));
        }
        let entry = entry.map_err(|_| invalid("cannot read verification observation entry"))?;
        if !entry
            .file_type()
            .map_err(|_| invalid("cannot inspect verification observation kind"))?
            .is_file()
        {
            return Err(invalid(
                "verification observations must be nonsymlink regular files",
            ));
        }
        names.push(
            entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("verification observation name must be UTF-8"))?,
        );
    }
    names.sort();
    let mut previous = None;
    for (index, name) in names.iter().enumerate() {
        cancelled(token)?;
        let sequence = index as u32 + 1;
        if *name != format!("{sequence:08}.json") {
            return Err(invalid(
                "verification observations must be contiguous fixed sequence names",
            ));
        }
        let path = directory.join(name);
        let observation: VerificationObservation = read_exact(&path, OBSERVATION_BYTES)?;
        if observation.schema != "aniflow.release-verification-observation/v1"
            || observation.sequence != sequence
            || observation.previous_observation_sha256 != previous
            || observation.clock_authority != CLOCK
            || observation.verification != *verification
        {
            return Err(invalid(
                "verification observation differs from its subject, sequence or previous exact observation digest",
            ));
        }
        previous = Some(local::hash(&local::bytes(&observation)?));
    }
    Ok((names.len() as u32, previous))
}
/// Append one bounded successful local verification observation. Existing
/// subjects and prior observations are never rewritten. This is no OTS proof.
pub fn record_verification(
    root: &Path,
    manifest_sha256: &str,
    token: &CancellationToken,
) -> Result<VerificationObservation> {
    cancelled(token)?;
    ensure_canonical_directory(root)?;
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let verification = storage::verify_inner(root, manifest_sha256, token, true)?;
    let (count, previous_observation_sha256) = chain(root, &verification, token)?;
    if count >= MAXIMUM_OBSERVATIONS {
        return Err(invalid(
            "verification observation limit reached; preserve history and reconcile explicitly",
        ));
    }
    let observation = VerificationObservation {
        schema: "aniflow.release-verification-observation/v1".into(),
        sequence: count + 1,
        previous_observation_sha256,
        observed_at: chrono::Utc::now(),
        clock_authority: CLOCK.into(),
        verification,
    };
    let bytes = local::bytes(&observation)?;
    if bytes.len() as u64 > OBSERVATION_BYTES {
        return Err(invalid("verification observation exceeds 16 KiB"));
    }
    let request: EvidenceRequest = read_exact(&root.join("request.json"), local::DOCUMENT_BYTES)?;
    crate::video_assembly::require_storage(root, bytes.len() as u64, request.minimum_free_bytes)?;
    cancelled(token)?;
    local::write(
        &root
            .join("observations")
            .join(format!("{:08}.json", observation.sequence)),
        &bytes,
    )?;
    Ok(observation)
}
