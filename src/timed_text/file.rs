//! Exclusive, source-preserving publication for timed-text conversions.
//!
//! The caller controls source and output parents during conversion. Identity
//! checks detect replacement; they are not a hostile-filesystem sandbox.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    ConversionFailure, ConversionOptions, ImportContext, MAX_EVIDENCE_BYTES, MAX_TEXT_BYTES,
    TimedTextConversion, TimedTextConversionReport, TimedTextDocument, TimedTextFormat, artifact,
    convert, decode, invalid, sha256,
};
use crate::audio_analysis::AudioArtifactReference;

/// A completed conversion bundle. `report_path` is published last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileConversionOutcome {
    pub output_directory: PathBuf,
    pub payload_path: PathBuf,
    pub input_document_path: PathBuf,
    pub output_document_path: PathBuf,
    pub report_path: PathBuf,
    pub report: TimedTextConversionReport,
}

/// Convert one bounded regular input into a newly reserved output directory.
/// Existing outputs are never replaced. No completion report is published
/// until every artifact is durable and the source has been reobserved.
pub fn convert_file(
    input: &Path,
    from: TimedTextFormat,
    to: TimedTextFormat,
    output_directory: &Path,
    context: Option<&ImportContext>,
    options: &ConversionOptions,
) -> Result<FileConversionOutcome, ConversionFailure> {
    let (_, outcome) = convert_bound_file(input, output_directory, to, MAX_TEXT_BYTES, |source| {
        let default_context = ImportContext::default();
        if from == TimedTextFormat::Json {
            if let Some(explicit) = context {
                explicit.validate()?;
                let embedded = decode(source, TimedTextFormat::Json, &default_context)?.document;
                if explicit.provenance != embedded.provenance
                    || explicit.language != embedded.language
                    || explicit.audio_source != embedded.audio_source
                    || explicit.overlap_policy != embedded.overlap_policy
                {
                    return Err(invalid(
                        "explicit context cannot reset or replace embedded JSON provenance or context",
                    )
                    .into());
                }
            }
        }
        convert(
            source,
            from,
            to,
            context.unwrap_or(&default_context),
            options,
        )
    })?;
    Ok(outcome)
}

/// Extract a validated embedded document while retaining and rechecking its
/// original envelope's bytes. Conversion evidence identifies the extracted
/// canonical JSON; the returned source reference identifies the envelope.
pub(crate) fn convert_embedded_file(
    input: &Path,
    to: TimedTextFormat,
    output_directory: &Path,
    options: &ConversionOptions,
    extract: impl FnOnce(&[u8]) -> crate::Result<TimedTextDocument>,
) -> Result<(AudioArtifactReference, FileConversionOutcome), ConversionFailure> {
    convert_bound_file(
        input,
        output_directory,
        to,
        MAX_EVIDENCE_BYTES as usize,
        |source| {
            let document = extract(source)?;
            convert(
                &document.canonical_json_bytes()?,
                TimedTextFormat::Json,
                to,
                &ImportContext::default(),
                options,
            )
        },
    )
}

fn convert_bound_file(
    input: &Path,
    output_directory: &Path,
    to: TimedTextFormat,
    maximum_source_bytes: usize,
    transform: impl FnOnce(&[u8]) -> Result<TimedTextConversion, ConversionFailure>,
) -> Result<(AudioArtifactReference, FileConversionOutcome), ConversionFailure> {
    let input = normalized_path(input)?;
    let output_directory = normalized_path(output_directory)?;
    let parent = output_directory
        .parent()
        .ok_or_else(|| invalid("conversion output requires an existing parent directory"))?;
    if output_directory.file_name().is_none()
        || fs::canonicalize(parent)
            .map_err(|_| invalid("conversion output parent is unavailable"))?
            != parent
    {
        return Err(invalid("conversion output parent must be canonical and symlink-free").into());
    }
    let source = read_source(&input, maximum_source_bytes)?;
    let source_reference = artifact("embedded_source", &source);
    let converted = transform(&source)?;
    let input_document = converted.input_document.canonical_json_bytes()?;
    let output_document = converted.output_document.canonical_json_bytes()?;
    let report_bytes = converted.report.canonical_json_bytes()?;

    // Reserve the actual destination atomically. There is no exists-check plus
    // rename sequence that could replace another caller's directory.
    let mut reservation = Reservation::new(&output_directory)?;
    let payload_path = output_directory.join(format!("payload.{}", to.extension()));
    let input_document_path = output_directory.join("normalized-input.json");
    let output_document_path = output_directory.join("normalized-output.json");
    let report_path = output_directory.join("conversion.json");
    reservation.write_new(&payload_path, &converted.bytes)?;
    reservation.write_new(&input_document_path, &input_document)?;
    reservation.write_new(&output_document_path, &output_document)?;
    reservation.write_completion(
        &report_path,
        &report_bytes,
        &input,
        &source_reference.sha256,
        maximum_source_bytes,
    )?;
    reservation.sync_directory()?;
    reservation.committed = true;
    Ok((
        source_reference,
        FileConversionOutcome {
            output_directory,
            payload_path,
            input_document_path,
            output_document_path,
            report_path,
            report: converted.report,
        },
    ))
}

fn normalized_path(path: &Path) -> crate::Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid(
            "conversion paths must be nonempty and must not contain dot traversal",
        ));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|_| invalid("conversion working directory is unavailable"))?
            .join(path))
    }
}

fn read_source(path: &Path, maximum_bytes: usize) -> crate::Result<Vec<u8>> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| invalid("conversion source is unavailable"))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > maximum_bytes as u64
        || fs::canonicalize(path).map_err(|_| invalid("conversion source cannot be resolved"))?
            != path
    {
        return Err(invalid(format!(
            "conversion source must be a canonical nonsymlink regular file of at most {maximum_bytes} bytes"
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // Refuse a replaced symlink and avoid blocking if the leaf becomes a
        // FIFO between inspection and opening. The opened identity is checked
        // below before any bytes are accepted.
        options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
    }
    let mut file = options
        .open(path)
        .map_err(|_| invalid("conversion source cannot be read"))?;
    let opened = file
        .metadata()
        .map_err(|_| invalid("conversion source metadata is unavailable"))?;
    if !opened.is_file() || !same_identity(&metadata, &opened) {
        return Err(invalid("conversion source changed while being opened"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    (&mut file)
        .take(maximum_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("conversion source could not be read completely"))?;
    let after = file
        .metadata()
        .map_err(|_| invalid("conversion source metadata is unavailable"))?;
    if bytes.is_empty()
        || bytes.len() > maximum_bytes
        || bytes.len() as u64 != metadata.len()
        || after.len() != metadata.len()
        || after.modified().ok() != metadata.modified().ok()
    {
        return Err(invalid(
            "conversion source changed or exceeded its bound during reading",
        ));
    }
    Ok(bytes)
}

struct Reservation {
    directory: PathBuf,
    identity: Metadata,
    files: Vec<(PathBuf, Metadata)>,
    committed: bool,
}

impl Reservation {
    fn new(directory: &Path) -> crate::Result<Self> {
        fs::create_dir(directory).map_err(|_| {
            invalid(
                "conversion destination must be a new directory; existing paths are never replaced",
            )
        })?;
        let identity = fs::symlink_metadata(directory)
            .map_err(|_| invalid("reserved conversion directory cannot be inspected"))?;
        if !identity.is_dir() || identity.file_type().is_symlink() {
            return Err(invalid("reserved conversion directory identity changed"));
        }
        Ok(Self {
            directory: directory.to_path_buf(),
            identity,
            files: Vec::new(),
            committed: false,
        })
    }

    fn still_owned(&self) -> bool {
        fs::symlink_metadata(&self.directory).is_ok_and(|current| {
            current.is_dir()
                && !current.file_type().is_symlink()
                && same_identity(&self.identity, &current)
        })
    }

    fn write_new(&mut self, path: &Path, bytes: &[u8]) -> crate::Result<()> {
        if !self.still_owned() {
            return Err(invalid("reserved conversion directory was replaced"));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| invalid("conversion artifact could not be created exclusively"))?;
        let identity = file
            .metadata()
            .map_err(|_| invalid("conversion artifact identity is unavailable"))?;
        self.files.push((path.to_path_buf(), identity));
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| invalid("conversion artifact could not be fully synchronized"))
    }

    fn sync_directory(&self) -> crate::Result<()> {
        if !self.still_owned() {
            return Err(invalid(
                "reserved conversion directory was replaced before completion",
            ));
        }
        #[cfg(unix)]
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| invalid("conversion directory could not be synchronized"))?;
        Ok(())
    }

    fn write_completion(
        &mut self,
        path: &Path,
        bytes: &[u8],
        source: &Path,
        source_digest: &str,
        maximum_source_bytes: usize,
    ) -> crate::Result<()> {
        if !self.still_owned() {
            return Err(invalid("reserved conversion directory was replaced"));
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".conversion-report-")
            .suffix(".tmp")
            .tempfile_in(&self.directory)
            .map_err(|_| invalid("conversion completion report could not be staged"))?;
        temporary
            .write_all(bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| invalid("conversion completion report could not be synchronized"))?;
        let identity = temporary
            .as_file()
            .metadata()
            .map_err(|_| invalid("conversion completion report identity is unavailable"))?;
        if sha256(&read_source(source, maximum_source_bytes)?) != source_digest {
            return Err(invalid(
                "conversion source changed during conversion; no completion report was accepted",
            ));
        }
        if !self.still_owned() {
            return Err(invalid(
                "reserved conversion directory was replaced before completion",
            ));
        }
        // Publish the complete, synchronized file in one no-clobber operation.
        // Readers can observe an absent report or its full bytes, never a
        // partially written completion marker.
        temporary.persist_noclobber(path).map_err(|_| {
            invalid("conversion completion report could not be published exclusively")
        })?;
        self.files.push((path.to_path_buf(), identity));
        Ok(())
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.committed || !self.still_owned() {
            return;
        }
        // Remove only file objects this reservation created. Never recursively
        // remove an unexpected entry or a directory swapped in by another actor.
        for (path, identity) in self.files.iter().rev() {
            if !self.still_owned() {
                return;
            }
            if fs::symlink_metadata(path).is_ok_and(|current| same_identity(identity, &current)) {
                let _ = fs::remove_file(path);
            }
        }
        if self.still_owned() {
            let _ = fs::remove_dir(&self.directory);
        }
    }
}

#[cfg(unix)]
fn same_identity(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_identity(left: &Metadata, right: &Metadata) -> bool {
    // When creation identity is unavailable, refuse cleanup/acceptance instead
    // of assuming an unrelated filesystem entry is owned by this conversion.
    left.created()
        .ok()
        .zip(right.created().ok())
        .is_some_and(|(left, right)| left == right)
        && left.file_type() == right.file_type()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory() -> (tempfile::TempDir, PathBuf) {
        let temporary = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(temporary.path()).unwrap();
        (temporary, canonical)
    }

    #[test]
    fn reservation_failure_removes_only_its_own_files() {
        let (_temporary, root) = temporary_directory();
        let destination = root.as_path().join("reserved");
        {
            let mut reservation = Reservation::new(&destination).unwrap();
            reservation
                .write_new(&destination.join("ours"), b"owned")
                .unwrap();
            fs::write(destination.join("foreign"), b"unrelated").unwrap();
        }
        assert!(!destination.join("ours").exists());
        assert_eq!(fs::read(destination.join("foreign")).unwrap(), b"unrelated");
    }

    #[test]
    fn completion_marker_is_never_replaced_or_published_for_a_changed_source() {
        let (_temporary, root) = temporary_directory();
        let source = root.as_path().join("source.txt");
        fs::write(&source, b"synthetic").unwrap();
        let digest = sha256(b"synthetic");
        let destination = root.as_path().join("reserved");
        let mut reservation = Reservation::new(&destination).unwrap();
        let report = destination.join("conversion.json");
        fs::write(&report, b"existing report").unwrap();
        assert!(
            reservation
                .write_completion(&report, b"new report", &source, &digest, MAX_TEXT_BYTES)
                .is_err()
        );
        assert_eq!(fs::read(&report).unwrap(), b"existing report");
        fs::remove_file(&report).unwrap();
        fs::write(&source, b"changed").unwrap();
        assert!(
            reservation
                .write_completion(&report, b"new report", &source, &digest, MAX_TEXT_BYTES)
                .is_err()
        );
        assert!(!report.exists());
        assert!(fs::read_dir(&destination).unwrap().next().is_none());
    }

    #[test]
    fn existing_output_is_untouched_and_source_bytes_remain_exact() {
        let (_temporary, root) = temporary_directory();
        let input = root.as_path().join("source.txt");
        fs::write(&input, "  Héllo 雪\n").unwrap();
        let destination = root.as_path().join("output");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), b"unchanged").unwrap();
        assert!(
            convert_file(
                &input,
                TimedTextFormat::Plain,
                TimedTextFormat::Json,
                &destination,
                None,
                &ConversionOptions::default()
            )
            .is_err()
        );
        assert_eq!(fs::read(destination.join("keep")).unwrap(), b"unchanged");
        assert_eq!(fs::read(&input).unwrap(), "  Héllo 雪\n".as_bytes());
    }

    #[test]
    fn explicit_default_context_cannot_reset_embedded_json_context() {
        let (_temporary, root) = temporary_directory();
        let context = ImportContext {
            language: Some("en".to_owned()),
            ..ImportContext::default()
        };
        let embedded = decode(b"words", TimedTextFormat::Plain, &context).unwrap();
        let input = root.as_path().join("input.json");
        fs::write(&input, embedded.document.canonical_json_bytes().unwrap()).unwrap();
        let refused = root.as_path().join("refused");
        assert!(
            convert_file(
                &input,
                TimedTextFormat::Json,
                TimedTextFormat::Json,
                &refused,
                Some(&ImportContext::default()),
                &ConversionOptions::default()
            )
            .is_err()
        );
        assert!(!refused.exists());
        let accepted = convert_file(
            &input,
            TimedTextFormat::Json,
            TimedTextFormat::Json,
            &root.as_path().join("accepted"),
            None,
            &ConversionOptions::default(),
        )
        .unwrap();
        assert!(accepted.report_path.is_file());
    }

    #[test]
    fn embedded_conversion_rechecks_the_original_envelope_before_completion() {
        let (_temporary, root) = temporary_directory();
        let input = root.join("envelope.json");
        let destination = root.join("export");
        fs::write(&input, b"synthetic original report").unwrap();
        let document = decode(
            b"observed words",
            TimedTextFormat::Plain,
            &ImportContext::default(),
        )
        .unwrap()
        .document;
        let error = convert_embedded_file(
            &input,
            TimedTextFormat::Json,
            &destination,
            &ConversionOptions::default(),
            |_| {
                fs::write(&input, b"synthetic changed report").unwrap();
                Ok(document)
            },
        )
        .unwrap_err();
        assert!(error.error.message().contains("changed during conversion"));
        assert!(!destination.exists());
        assert_eq!(fs::read(&input).unwrap(), b"synthetic changed report");
    }

    #[test]
    fn envelope_bound_does_not_widen_text_input_and_keeps_identities_distinct() {
        let (_temporary, root) = temporary_directory();
        let input = root.join("envelope.json");
        let source = vec![b'x'; MAX_TEXT_BYTES + 1];
        fs::write(&input, &source).unwrap();
        let refused = root.join("ordinary");
        assert!(
            convert_file(
                &input,
                TimedTextFormat::Plain,
                TimedTextFormat::Json,
                &refused,
                None,
                &ConversionOptions::default(),
            )
            .is_err()
        );
        assert!(!refused.exists());
        let document = decode(
            b"observed words",
            TimedTextFormat::Plain,
            &ImportContext::default(),
        )
        .unwrap()
        .document;
        let embedded_bytes = document.canonical_json_bytes().unwrap();
        let (source_reference, outcome) = convert_embedded_file(
            &input,
            TimedTextFormat::Json,
            &root.join("embedded"),
            &ConversionOptions::default(),
            |_| Ok(document.clone()),
        )
        .unwrap();
        assert_eq!(source_reference.byte_size, source.len() as u64);
        assert_eq!(source_reference.sha256, sha256(&source));
        assert_eq!(outcome.report.input.sha256, sha256(&embedded_bytes));
        assert_ne!(source_reference.sha256, outcome.report.input.sha256);
        assert_eq!(fs::read(&input).unwrap(), source);

        let oversized = File::create(&input).unwrap();
        oversized.set_len(MAX_EVIDENCE_BYTES + 1).unwrap();
        let destination = root.join("oversized");
        assert!(
            convert_embedded_file(
                &input,
                TimedTextFormat::Json,
                &destination,
                &ConversionOptions::default(),
                |_| panic!("oversized source must not reach extraction"),
            )
            .is_err()
        );
        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_source_and_output_parent_are_refused_without_publication() {
        use std::os::unix::fs::symlink;
        let (_temporary, root) = temporary_directory();
        let source = root.as_path().join("source.txt");
        fs::write(&source, b"synthetic").unwrap();
        let link = root.as_path().join("source-link.txt");
        symlink(&source, &link).unwrap();
        let destination = root.as_path().join("output");
        assert!(
            convert_file(
                &link,
                TimedTextFormat::Plain,
                TimedTextFormat::Json,
                &destination,
                None,
                &ConversionOptions::default()
            )
            .is_err()
        );
        assert!(!destination.exists());
        let parent_link = root.as_path().join("parent-link");
        symlink(root.as_path(), &parent_link).unwrap();
        assert!(
            convert_file(
                &source,
                TimedTextFormat::Plain,
                TimedTextFormat::Json,
                &parent_link.join("outside"),
                None,
                &ConversionOptions::default()
            )
            .is_err()
        );
        assert!(!root.as_path().join("outside").exists());
    }
}
