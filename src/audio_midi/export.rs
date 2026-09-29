//! Source-preserving publication of a candidate MIDI and its evidence companions.
use super::midi::{MIDI_MAXIMUM_BYTES, MidiReadBack, encode_midi, read_midi};
use super::types::*;
use crate::Result;
use crate::audio_analysis::{AudioArtifactReference, AudioRationalTime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub const AUDIO_MIDI_EXPORT_SCHEMA_V1: &str = "aniflow.audio-midi-export/v1";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiCandidateAuthority {
    Candidate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiExportMapping {
    NativeTimesRoundedToMicroseconds,
    CandidateTimesRoundedToMidiTicks,
    ActivationRetainedInCompanion,
    PlaceholderChannelAndProgram,
    PitchBendsUnavailable,
    FixedTempoIsClockOnly,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiExportProfile {
    pub format: u16,
    pub tracks: u16,
    pub ticks_per_quarter: u16,
    pub microseconds_per_quarter: u32,
    pub channel: u8,
    pub program: u8,
    pub tick_rounding: String,
    pub maximum_tick_error_seconds: AudioRationalTime,
    pub inferred_source_tempo: bool,
    pub inferred_source_instrument: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiExportReport {
    pub schema: String,
    pub authority: MidiCandidateAuthority,
    pub source_report: AudioArtifactReference,
    pub source_audio: AudioArtifactReference,
    pub notes: AudioArtifactReference,
    pub midi: AudioArtifactReference,
    pub profile: MidiExportProfile,
    pub mappings: Vec<MidiExportMapping>,
    pub read_back: MidiReadBack,
    pub note_count: u64,
    pub empty_candidate: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiExportOutcome {
    pub output_directory: PathBuf,
    pub midi_path: PathBuf,
    pub candidate_report_path: PathBuf,
    pub notes_path: PathBuf,
    pub report_path: PathBuf,
    pub report: MidiExportReport,
}
impl Default for MidiExportProfile {
    fn default() -> Self {
        Self {
            format: 0,
            tracks: 1,
            ticks_per_quarter: 960,
            microseconds_per_quarter: 500000,
            channel: 0,
            program: 0,
            tick_rounding: "nearest_tick_ties_up".into(),
            maximum_tick_error_seconds: AudioRationalTime {
                numerator: 1,
                denominator: 3840,
            },
            inferred_source_tempo: false,
            inferred_source_instrument: false,
        }
    }
}
impl MidiExportMapping {
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![
            Self::NativeTimesRoundedToMicroseconds,
            Self::CandidateTimesRoundedToMidiTicks,
            Self::ActivationRetainedInCompanion,
            Self::PlaceholderChannelAndProgram,
            Self::PitchBendsUnavailable,
            Self::FixedTempoIsClockOnly,
        ]
    }
}
impl MidiExportReport {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, AUDIO_MIDI_MAXIMUM_REPORT_BYTES, "MIDI export report")?;
        let value: Self = crate::provider::decode_json(bytes, "MIDI export report")?;
        value.validate()?;
        Ok(value)
    }
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        crate::provider::canonical_json_bytes(self)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_MIDI_EXPORT_SCHEMA_V1
            || self.authority != MidiCandidateAuthority::Candidate
            || self.profile != MidiExportProfile::default()
            || self.mappings != MidiExportMapping::all()
            || self.note_count != self.read_back.notes.len() as u64
            || self.empty_candidate != (self.note_count == 0)
        {
            return Err(invalid(
                "MIDI export evidence contradicts its declared profile or note count",
            ));
        }
        for (reference_value, id, maximum) in [
            (
                &self.source_report,
                "midi",
                AUDIO_MIDI_MAXIMUM_REPORT_BYTES as u64,
            ),
            (&self.source_audio, "source_audio", 256 * 1024 * 1024),
            (
                &self.notes,
                "midi_notes",
                AUDIO_MIDI_MAXIMUM_REPORT_BYTES as u64,
            ),
            (&self.midi, "midi_candidate", MIDI_MAXIMUM_BYTES as u64),
        ] {
            reference(reference_value, maximum)?;
            if reference_value.id != id {
                return Err(invalid("MIDI export artifact identity is unexpected"));
            }
        }
        self.read_back.validate()
    }
}
/// Export only an explicit validated candidate. Existing paths are never
/// replaced. `export-report.json` is the last, atomic completion marker; without
/// that marker an interrupted directory is incomplete and must not be consumed.
pub fn export_midi_file(input: &Path, output_directory: &Path) -> Result<MidiExportOutcome> {
    let input = normalized_path(input)?;
    let output_directory = normalized_path(output_directory)?;
    let parent = output_directory
        .parent()
        .ok_or_else(|| invalid("MIDI export output requires an existing parent"))?;
    if output_directory.file_name().is_none()
        || fs::canonicalize(parent).map_err(|_| invalid("MIDI export parent unavailable"))?
            != parent
    {
        return Err(invalid(
            "MIDI export parent must be canonical and symlink-free",
        ));
    }
    let source = read_source(&input, AUDIO_MIDI_MAXIMUM_REPORT_BYTES)?;
    let candidate = AudioMidiReport::from_json_slice(&source)?;
    let MidiResult::Candidate { observation } = &candidate.result else {
        return Err(invalid(
            "unavailable MIDI report has no candidate to export",
        ));
    };
    let midi = encode_midi(observation, &candidate.source)?;
    let notes = crate::provider::canonical_json_bytes(observation)?;
    let report = MidiExportReport {
        schema: AUDIO_MIDI_EXPORT_SCHEMA_V1.into(),
        authority: MidiCandidateAuthority::Candidate,
        source_report: artifact("midi", &source),
        source_audio: candidate.source.artifact.clone(),
        notes: artifact("midi_notes", &notes),
        midi: artifact("midi_candidate", &midi),
        profile: MidiExportProfile::default(),
        mappings: MidiExportMapping::all(),
        read_back: read_midi(&midi)?,
        note_count: observation.notes.len() as u64,
        empty_candidate: observation.notes.is_empty(),
    };
    let report_bytes = report.canonical_json_bytes()?;
    let mut reservation = Reservation::new(&output_directory)?;
    let midi_path = output_directory.join("candidate.mid");
    let candidate_report_path = output_directory.join("candidate-report.json");
    let notes_path = output_directory.join("notes.json");
    let report_path = output_directory.join("export-report.json");
    reservation.write_new(&midi_path, &midi)?;
    reservation.write_new(&candidate_report_path, &source)?;
    reservation.write_new(&notes_path, &notes)?;
    reservation.write_completion(
        &report_path,
        &report_bytes,
        &input,
        &report.source_report.sha256,
        AUDIO_MIDI_MAXIMUM_REPORT_BYTES,
    )?;
    reservation.sync_directory()?;
    reservation.committed = true;
    Ok(MidiExportOutcome {
        output_directory,
        midi_path,
        candidate_report_path,
        notes_path,
        report_path,
        report,
    })
}
fn artifact(id: &str, bytes: &[u8]) -> AudioArtifactReference {
    AudioArtifactReference {
        id: id.into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        byte_size: bytes.len() as u64,
    }
}
fn normalized_path(path: &Path) -> crate::Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid(
            "MIDI export paths must be nonempty and must not contain dot traversal",
        ));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|_| invalid("MIDI export working directory is unavailable"))?
            .join(path))
    }
}

fn read_source(path: &Path, maximum_bytes: usize) -> crate::Result<Vec<u8>> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| invalid("MIDI export source is unavailable"))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > maximum_bytes as u64
        || fs::canonicalize(path).map_err(|_| invalid("MIDI export source cannot be resolved"))?
            != path
    {
        return Err(invalid(format!(
            "MIDI export source must be a canonical nonsymlink regular file of at most {maximum_bytes} bytes"
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
        .map_err(|_| invalid("MIDI export source cannot be read"))?;
    let opened = file
        .metadata()
        .map_err(|_| invalid("MIDI export source metadata is unavailable"))?;
    if !opened.is_file() || !same_identity(&metadata, &opened) {
        return Err(invalid("MIDI export source changed while being opened"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    (&mut file)
        .take(maximum_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("MIDI export source could not be read completely"))?;
    let after = file
        .metadata()
        .map_err(|_| invalid("MIDI export source metadata is unavailable"))?;
    if bytes.is_empty()
        || bytes.len() > maximum_bytes
        || bytes.len() as u64 != metadata.len()
        || after.len() != metadata.len()
        || after.modified().ok() != metadata.modified().ok()
    {
        return Err(invalid(
            "MIDI export source changed or exceeded its bound during reading",
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
                "MIDI export destination must be a new directory; existing paths are never replaced",
            )
        })?;
        let identity = fs::symlink_metadata(directory)
            .map_err(|_| invalid("reserved MIDI export directory cannot be inspected"))?;
        if !identity.is_dir() || identity.file_type().is_symlink() {
            return Err(invalid("reserved MIDI export directory identity changed"));
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
            return Err(invalid("reserved MIDI export directory was replaced"));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| invalid("MIDI export artifact could not be created exclusively"))?;
        let identity = file
            .metadata()
            .map_err(|_| invalid("MIDI export artifact identity is unavailable"))?;
        self.files.push((path.to_path_buf(), identity));
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| invalid("MIDI export artifact could not be fully synchronized"))
    }

    fn sync_directory(&self) -> crate::Result<()> {
        if !self.still_owned() {
            return Err(invalid(
                "reserved MIDI export directory was replaced before completion",
            ));
        }
        #[cfg(unix)]
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| invalid("MIDI export directory could not be synchronized"))?;
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
            return Err(invalid("reserved MIDI export directory was replaced"));
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".midi-export-report-")
            .suffix(".tmp")
            .tempfile_in(&self.directory)
            .map_err(|_| invalid("MIDI export completion report could not be staged"))?;
        temporary
            .write_all(bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| invalid("MIDI export completion report could not be synchronized"))?;
        let identity = temporary
            .as_file()
            .metadata()
            .map_err(|_| invalid("MIDI export completion report identity is unavailable"))?;
        if sha256(&read_source(source, maximum_source_bytes)?) != source_digest {
            return Err(invalid(
                "MIDI export source changed during MIDI export; no completion report was accepted",
            ));
        }
        if !self.still_owned() {
            return Err(invalid(
                "reserved MIDI export directory was replaced before completion",
            ));
        }
        // Publish the complete, synchronized file in one no-clobber operation.
        // Readers can observe an absent report or its full bytes, never a
        // partially written completion marker.
        temporary.persist_noclobber(path).map_err(|_| {
            invalid("MIDI export completion report could not be published exclusively")
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
    // of assuming an unrelated filesystem entry is owned by this MIDI export.
    left.created()
        .ok()
        .zip(right.created().ok())
        .is_some_and(|(left, right)| left == right)
        && left.file_type() == right.file_type()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> AudioMidiReport {
        AudioMidiReport::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/audio-midi-v1.example.json"
        ))
        .unwrap()
    }
    #[test]
    fn export_keeps_source_exact_and_publishes_consistent_complete_set() {
        let temporary = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temporary.path()).unwrap();
        let input = root.join("candidate.json");
        let output = root.join("export");
        let report = fixture();
        let source = serde_json::to_vec_pretty(&report).unwrap();
        fs::write(&input, &source).unwrap();
        let result = export_midi_file(&input, &output).unwrap();
        assert_eq!(fs::read(&input).unwrap(), source);
        assert_eq!(fs::read(&result.candidate_report_path).unwrap(), source);
        assert_eq!(
            AudioMidiReport::from_json_slice(&fs::read(&result.candidate_report_path).unwrap())
                .unwrap(),
            report
        );
        let midi = fs::read(&result.midi_path).unwrap();
        assert_eq!(read_midi(&midi).unwrap(), result.report.read_back);
        assert_eq!(artifact("midi_candidate", &midi), result.report.midi);
        assert_eq!(
            MidiExportReport::from_json_slice(&fs::read(&result.report_path).unwrap()).unwrap(),
            result.report
        );
        assert_eq!(fs::read_dir(&output).unwrap().count(), 4);
        assert!(export_midi_file(&input, &output).is_err());
        assert_eq!(fs::read(&result.midi_path).unwrap(), midi);
    }
    #[test]
    fn malformed_candidate_cannot_reserve_output() {
        let temporary = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temporary.path()).unwrap();
        let input = root.join("candidate.json");
        let output = root.join("export");
        fs::write(&input, b"{}").unwrap();
        assert!(export_midi_file(&input, &output).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), b"{}");
    }
    #[test]
    fn empty_candidate_exports_no_notes_with_explicit_empty_evidence() {
        let temporary = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temporary.path()).unwrap();
        let input = root.join("candidate.json");
        let output = root.join("export");
        let mut report = fixture();
        let MidiResult::Candidate { observation } = &mut report.result else {
            panic!("candidate fixture")
        };
        observation.native.notes.clear();
        observation.notes.clear();
        fs::write(&input, report.canonical_json_bytes().unwrap()).unwrap();
        let result = export_midi_file(&input, &output).unwrap();
        assert_eq!(result.report.note_count, 0);
        assert!(result.report.empty_candidate);
        assert_eq!(fs::metadata(&result.midi_path).unwrap().len(), 36);
    }
    #[test]
    fn output_reservation_and_changed_source_cannot_publish_completion() {
        let temporary = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temporary.path()).unwrap();
        let input = root.join("input");
        fs::write(&input, b"initial").unwrap();
        let output = root.join("output");
        {
            let mut reservation = Reservation::new(&output).unwrap();
            reservation
                .write_new(&output.join("candidate.mid"), b"synthetic")
                .unwrap();
            fs::write(&input, b"changed").unwrap();
            assert!(
                reservation
                    .write_completion(
                        &output.join("export-report.json"),
                        b"{}",
                        &input,
                        &sha256(b"initial"),
                        1024
                    )
                    .is_err()
            );
            assert!(!output.join("export-report.json").exists());
        }
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), b"changed");
    }
    #[cfg(unix)]
    #[test]
    fn symlink_input_or_parent_is_refused_without_output() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temporary.path()).unwrap();
        let input = root.join("candidate.json");
        fs::write(&input, fixture().canonical_json_bytes().unwrap()).unwrap();
        let link = root.join("linked.json");
        symlink(&input, &link).unwrap();
        assert!(export_midi_file(&link, &root.join("export")).is_err());
        assert!(!root.join("export").exists());
        let parent = root.join("linked-parent");
        symlink(&root, &parent).unwrap();
        assert!(export_midi_file(&input, &parent.join("export")).is_err());
        assert!(!root.join("export").exists());
    }
}
