//! Small bounded local evidence I/O shared by delivery preparation.
use super::invalid;
use crate::{
    Result,
    frame_io::{ensure_canonical_directory, identity, require_absent},
};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
};
pub(crate) const DOCUMENT_BYTES: u64 = 2_097_152;
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = crate::provider::canonical_json_bytes(value)?;
    if bytes.len() as u64 > DOCUMENT_BYTES {
        return Err(invalid("local delivery evidence exceeds 2 MiB"));
    }
    Ok(bytes)
}
pub(crate) fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    read_bounded(path, DOCUMENT_BYTES)
}
pub(crate) fn read_bounded<T: DeserializeOwned>(path: &Path, maximum: u64) -> Result<T> {
    let before = identity(path, maximum)?;
    let mut bytes = Vec::new();
    crate::audio_inspection::process::open_regular(path)
        .map_err(|_| invalid("cannot open private delivery evidence"))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("cannot read bounded delivery evidence"))?;
    if (hash(&bytes), bytes.len() as u64) != before || identity(path, maximum)? != before {
        return Err(invalid("delivery evidence changed during read"));
    }
    crate::provider::decode_json(&bytes, "local delivery evidence")
}
pub(crate) fn sync(directory: &Path) -> Result<()> {
    fs::File::open(directory)
        .and_then(|f| f.sync_all())
        .map_err(|_| invalid("cannot sync delivery directory"))
}
pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if bytes.len() as u64 > DOCUMENT_BYTES {
        return Err(invalid("delivery document exceeds 2 MiB"));
    }
    ensure_canonical_directory(
        path.parent()
            .ok_or_else(|| invalid("delivery evidence parent missing"))?,
    )?;
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| invalid("delivery evidence already exists or cannot be reserved"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| invalid("cannot sync delivery evidence"))?;
    sync(
        path.parent()
            .ok_or_else(|| invalid("delivery evidence parent missing"))?,
    )
}
pub(crate) fn directory(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|_| invalid("private delivery directory exists or cannot be reserved"))?;
    ensure_canonical_directory(path)
}
pub(crate) fn parent(root: &Path) -> Result<&Path> {
    if !cfg!(unix)
        || !root.is_absolute()
        || root
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid(
            "initial local evidence writer requires Unix and normalized absolute paths",
        ));
    }
    let parent = root
        .parent()
        .ok_or_else(|| invalid("delivery output needs an existing parent"))?;
    ensure_canonical_directory(parent)?;
    require_absent(root)?;
    Ok(parent)
}
pub(crate) fn disjoint(root: &Path, input: &super::MediaInput, file: &Path) -> Result<()> {
    let run = match input {
        super::MediaInput::AcceptedVideo { run_directory, .. }
        | super::MediaInput::AcceptedVideoWithQc { run_directory, .. }
        | super::MediaInput::AcceptedAudioExport { run_directory }
        | super::MediaInput::AcceptedGainCandidate { run_directory } => run_directory,
        super::MediaInput::AcceptedPcm {
            signal_run_directory,
            ..
        } => signal_run_directory,
        super::MediaInput::AcceptedAudioWithQc { subject, .. } => subject.run_directory(),
    };
    if let super::MediaInput::AcceptedVideoWithQc { qc_directory, .. }
    | super::MediaInput::AcceptedAudioWithQc { qc_directory, .. } = input
    {
        if root.starts_with(qc_directory) || qc_directory.starts_with(root) {
            return Err(invalid(
                "delivery output must be separate from current final-file QC",
            ));
        }
    }
    if root.starts_with(run)
        || run.starts_with(root)
        || file.starts_with(root)
        || root.starts_with(file)
    {
        return Err(invalid(
            "delivery output must be separate from source media and its accepted run",
        ));
    }
    Ok(())
}
pub(crate) fn entries(
    root: &Path,
    files: &[&str],
    directories: &[&str],
    allow_writer: bool,
) -> Result<()> {
    use std::collections::BTreeSet;
    ensure_canonical_directory(root)?;
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(root).map_err(|_| invalid("cannot inspect delivery inventory"))? {
        if actual.len() > files.len() + directories.len() {
            return Err(invalid("delivery inventory exceeds its fixed membership"));
        }
        let entry = entry.map_err(|_| invalid("cannot read delivery member"))?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| invalid("delivery member name is not UTF-8"))?;
        let kind = entry
            .file_type()
            .map_err(|_| invalid("cannot inspect delivery member kind"))?;
        if (name == "writer.lock" && allow_writer && kind.is_file())
            || files.contains(&name) && kind.is_file()
            || directories.contains(&name) && kind.is_dir()
        {
            actual.insert(name.to_owned());
        } else {
            return Err(invalid(
                "delivery inventory has an undeclared, symbolic-link or special member",
            ));
        }
    }
    actual.remove("writer.lock");
    let expected: BTreeSet<_> = files
        .iter()
        .chain(directories)
        .map(|s| s.to_string())
        .collect();
    if actual != expected {
        return Err(invalid("delivery inventory has missing members"));
    }
    Ok(())
}
