//! A deliberately closed Standard MIDI File type 0 writer and independent reader.
//! This is an export format, not an authored-MIDI import or editing API.
use super::types::{AUDIO_MIDI_MAXIMUM_NOTES, MidiObservation, invalid};
use crate::{Result, audio_analysis::AudioSource};
use serde::{Deserialize, Serialize};

pub const MIDI_TICKS_PER_QUARTER: u16 = 960;
pub const MIDI_MICROSECONDS_PER_QUARTER: u32 = 500000;
pub const MIDI_MAXIMUM_BYTES: usize = 256 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiReadBackNote {
    pub start_tick: u32,
    pub end_tick: u32,
    pub pitch: u8,
    pub velocity: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiReadBack {
    pub format: u16,
    pub tracks: u16,
    pub ticks_per_quarter: u16,
    pub microseconds_per_quarter: u32,
    pub channel: u8,
    pub program: u8,
    pub notes: Vec<MidiReadBackNote>,
}
/// Emit explicit statuses, note-off before note-on at a shared tick, a single
/// fixed tempo and a declared placeholder program/channel. No facts about the
/// source's actual tempo, instrument or musical meter are inferred here.
pub fn encode_midi(observation: &MidiObservation, source: &AudioSource) -> Result<Vec<u8>> {
    observation.validate(source)?;
    let mut events = Vec::with_capacity(observation.notes.len() * 2);
    for note in &observation.notes {
        events.push((note.start_tick, 1u8, note.pitch, note.velocity));
        events.push((note.end_tick, 0u8, note.pitch, 0u8));
    }
    events.sort_unstable();
    let mut track = vec![0, 0xff, 0x51, 3, 0x07, 0xa1, 0x20, 0, 0xc0, 0];
    let mut previous = 0;
    for (tick, kind, pitch, velocity) in events {
        write_vlq(tick - previous, &mut track);
        track.extend([if kind == 0 { 0x80 } else { 0x90 }, pitch, velocity]);
        previous = tick;
    }
    track.extend([0, 0xff, 0x2f, 0]);
    let mut bytes = b"MThd\0\0\0\x06\0\0\0\x01\x03\xc0MTrk".to_vec();
    bytes.extend((track.len() as u32).to_be_bytes());
    bytes.extend(track);
    if bytes.len() > MIDI_MAXIMUM_BYTES {
        return Err(invalid("MIDI output exceeds its byte bound"));
    }
    let readback = read_midi(&bytes)?;
    let mut expected = observation
        .notes
        .iter()
        .map(|n| MidiReadBackNote {
            start_tick: n.start_tick,
            end_tick: n.end_tick,
            pitch: n.pitch,
            velocity: n.velocity,
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|n| (n.start_tick, n.pitch, n.end_tick));
    if readback.notes != expected {
        return Err(invalid(
            "independent MIDI read-back differs from every normalized note",
        ));
    }
    Ok(bytes)
}
fn write_vlq(mut value: u32, output: &mut Vec<u8>) {
    let mut buffer = [0; 4];
    let mut index = 3;
    buffer[index] = (value & 0x7f) as u8;
    while {
        value >>= 7;
        value != 0
    } {
        index -= 1;
        buffer[index] = ((value & 0x7f) | 0x80) as u8;
    }
    output.extend(&buffer[index..]);
}
/// Parse bytes independently of the writer. Reject everything outside the
/// published subset, including running status, SMPTE clocks, pitch bends,
/// extra tracks, altered tempo/program, duplicate events and trailing bytes.
pub fn read_midi(bytes: &[u8]) -> Result<MidiReadBack> {
    if bytes.len() < 36
        || bytes.len() > MIDI_MAXIMUM_BYTES
        || &bytes[..18] != b"MThd\0\0\0\x06\0\0\0\x01\x03\xc0MTrk"
    {
        return Err(invalid(
            "MIDI header is outside the closed type 0/960 PPQ subset",
        ));
    }
    let track_length = u32::from_be_bytes(bytes[18..22].try_into().expect("four bytes")) as usize;
    if track_length != bytes.len() - 22 {
        return Err(invalid("MIDI track length or trailing bytes invalid"));
    }
    let track = &bytes[22..];
    if !track.starts_with(&[0, 0xff, 0x51, 3, 0x07, 0xa1, 0x20, 0, 0xc0, 0]) {
        return Err(invalid(
            "MIDI requires exactly the fixed tempo and placeholder program prefix",
        ));
    }
    let mut index = 10;
    let mut absolute_tick = 0u32;
    let mut previous_key = None;
    let mut active = [None; 128];
    let mut notes = Vec::new();
    let mut events = 0usize;
    loop {
        let delta = read_vlq(track, &mut index)?;
        absolute_tick = absolute_tick
            .checked_add(delta)
            .ok_or_else(|| invalid("MIDI absolute tick overflow"))?;
        if absolute_tick > 230400 {
            return Err(invalid("MIDI tick exceeds 120 second profile"));
        }
        let status = *track
            .get(index)
            .ok_or_else(|| invalid("MIDI event is truncated"))?;
        index += 1;
        if status == 0xff {
            if delta != 0
                || track.get(index..index + 2) != Some(&[0x2f, 0][..])
                || index + 2 != track.len()
                || active.iter().any(Option::is_some)
            {
                return Err(invalid(
                    "MIDI end-of-track is invalid, early, or leaves active notes",
                ));
            }
            break;
        }
        let payload = track
            .get(index..index + 2)
            .ok_or_else(|| invalid("MIDI note event truncated"))?;
        index += 2;
        let pitch = payload[0];
        let velocity = payload[1];
        if !(21..=108).contains(&pitch)
            || !matches!(status, 0x80 | 0x90)
            || (status == 0x80 && velocity != 0)
            || (status == 0x90 && !(1..=127).contains(&velocity))
        {
            return Err(invalid("unsupported MIDI status, pitch or velocity"));
        }
        let key = (absolute_tick, u8::from(status == 0x90), pitch);
        if previous_key.is_some_and(|old| old >= key) {
            return Err(invalid(
                "MIDI events violate strict tick/off/on/pitch ordering",
            ));
        }
        previous_key = Some(key);
        let slot = &mut active[usize::from(pitch)];
        if status == 0x90 {
            if slot.is_some() {
                return Err(invalid("MIDI same-pitch retrigger is ambiguous"));
            }
            *slot = Some((absolute_tick, velocity));
        } else {
            let (start_tick, start_velocity) = slot
                .take()
                .ok_or_else(|| invalid("MIDI note-off has no matching note-on"))?;
            if start_tick >= absolute_tick {
                return Err(invalid("MIDI note has no positive duration"));
            }
            notes.push(MidiReadBackNote {
                start_tick,
                end_tick: absolute_tick,
                pitch,
                velocity: start_velocity,
            });
        }
        events += 1;
        if events > AUDIO_MIDI_MAXIMUM_NOTES * 2 {
            return Err(invalid("MIDI event count exceeds profile"));
        }
    }
    notes.sort_by_key(|n| (n.start_tick, n.pitch, n.end_tick));
    Ok(MidiReadBack {
        format: 0,
        tracks: 1,
        ticks_per_quarter: 960,
        microseconds_per_quarter: 500000,
        channel: 0,
        program: 0,
        notes,
    })
}
fn read_vlq(bytes: &[u8], index: &mut usize) -> Result<u32> {
    let mut value = 0u32;
    for count in 0..4 {
        let byte = *bytes
            .get(*index)
            .ok_or_else(|| invalid("MIDI delta is truncated"))?;
        *index += 1;
        if count == 0 && byte == 0x80 {
            return Err(invalid("MIDI delta has a noncanonical leading zero"));
        }
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(invalid("MIDI delta exceeds four bytes"))
}

impl MidiReadBack {
    pub fn validate(&self) -> Result<()> {
        if self.format != 0
            || self.tracks != 1
            || self.ticks_per_quarter != 960
            || self.microseconds_per_quarter != 500000
            || self.channel != 0
            || self.program != 0
            || self.notes.len() > AUDIO_MIDI_MAXIMUM_NOTES
        {
            return Err(invalid("MIDI read-back has an unsupported profile"));
        }
        let mut previous = None;
        let mut ends = [0; 128];
        for note in &self.notes {
            if !(21..=108).contains(&note.pitch)
                || !(1..=127).contains(&note.velocity)
                || note.start_tick >= note.end_tick
                || note.end_tick > 230400
            {
                return Err(invalid("MIDI read-back note is outside bounded profile"));
            }
            let key = (note.start_tick, note.pitch, note.end_tick);
            if previous.is_some_and(|old| old >= key)
                || note.start_tick < ends[usize::from(note.pitch)]
            {
                return Err(invalid(
                    "MIDI read-back note ordering or same-pitch overlap is invalid",
                ));
            }
            previous = Some(key);
            ends[usize::from(note.pitch)] = note.end_tick;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::*;
    use super::*;
    use crate::audio_analysis::{AudioArtifactReference, AudioRationalTime};
    fn fixture() -> (MidiObservation, AudioSource) {
        let source = AudioSource {
            artifact: AudioArtifactReference {
                id: "source_audio".into(),
                sha256: "a".repeat(64),
                byte_size: 88244,
            },
            stream_index: 0,
            sample_rate_hz: 22050,
            channels: 1,
            frame_count: 44100,
            origin: AudioRationalTime {
                numerator: 0,
                denominator: 1,
            },
            stem: None,
        };
        let native = AudioMidiNativeObservation {
            schema: AUDIO_MIDI_OBSERVATION_SCHEMA_V1.into(),
            sample_rate_hz: 22050,
            channels: 1,
            sample_frames: 44100,
            duration_microseconds: 2000000,
            notes: vec![
                MidiNativeNote {
                    start_microseconds: 0,
                    end_microseconds: 1000000,
                    pitch: 60,
                    velocity: 64,
                    activation: 0.5,
                },
                MidiNativeNote {
                    start_microseconds: 500000,
                    end_microseconds: 1500000,
                    pitch: 64,
                    velocity: 127,
                    activation: 1.0,
                },
                MidiNativeNote {
                    start_microseconds: 1000000,
                    end_microseconds: 2000000,
                    pitch: 60,
                    velocity: 64,
                    activation: 0.5,
                },
            ],
        };
        (
            super::super::normalize::from_native(native, &source).unwrap(),
            source,
        )
    }
    #[test]
    fn writer_and_reader_preserve_polyphony_retrigger_and_every_note() {
        let (o, s) = fixture();
        let bytes = encode_midi(&o, &s).unwrap();
        let result = read_midi(&bytes).unwrap();
        assert_eq!(result.notes.len(), 3);
        assert_eq!(result.notes[2].start_tick, 1920);
        assert_eq!(result.notes[2].pitch, 60);
    }
    #[test]
    fn distinct_native_onsets_that_share_a_tick_keep_both_notes() {
        let (mut observation, source) = fixture();
        observation.native.notes.truncate(2);
        observation.native.notes[0].start_microseconds = 100;
        observation.native.notes[0].pitch = 80;
        observation.native.notes[1].start_microseconds = 200;
        observation.native.notes[1].pitch = 60;
        let observation =
            super::super::normalize::from_native(observation.native, &source).unwrap();
        let bytes = encode_midi(&observation, &source).unwrap();
        let parsed = read_midi(&bytes).unwrap();
        assert_eq!(parsed.notes.len(), 2);
        assert_eq!(parsed.notes[0].pitch, 60);
        assert_eq!(parsed.notes[1].pitch, 80);
        assert!(parsed.notes.iter().all(|n| n.start_tick == 0));
    }
    #[test]
    fn empty_candidate_is_a_valid_explicitly_empty_track() {
        let (mut o, s) = fixture();
        o.native.notes.clear();
        o.notes.clear();
        let bytes = encode_midi(&o, &s).unwrap();
        assert!(read_midi(&bytes).unwrap().notes.is_empty());
        assert_eq!(bytes.len(), 36);
    }
    #[test]
    fn reader_refuses_mutated_header_tempo_status_length_and_end() {
        let (o, s) = fixture();
        let bytes = encode_midi(&o, &s).unwrap();
        for (offset, value) in [
            (9, 1),
            (11, 2),
            (12, 0x80),
            (28, 0x21),
            (33, 0x91),
            (34, 0xff),
            (35, 0),
        ] {
            let mut b = bytes.clone();
            b[offset] = value;
            assert!(read_midi(&b).is_err(), "offset {offset}");
        }
        let mut b = bytes.clone();
        b.push(0);
        assert!(read_midi(&b).is_err());
        let mut b = bytes;
        b.pop();
        assert!(read_midi(&b).is_err());
    }
    #[test]
    fn reader_refuses_running_status_pitch_bends_and_unmatched_note_off() {
        let (o, s) = fixture();
        let bytes = encode_midi(&o, &s).unwrap();
        for status in [60, 0xe0, 0x80, 0x99] {
            let mut b = bytes.clone();
            b[33] = status;
            assert!(read_midi(&b).is_err());
        }
    }
    #[test]
    fn vlq_boundaries_have_canonical_roundtrip_and_overlong_is_refused() {
        for value in [0, 127, 128, 16383, 16384, 230400] {
            let mut b = Vec::new();
            write_vlq(value, &mut b);
            let mut i = 0;
            assert_eq!(read_vlq(&b, &mut i).unwrap(), value);
            assert_eq!(i, b.len());
        }
        assert!(read_vlq(&[0x80, 0], &mut 0).is_err());
        assert!(read_vlq(&[0xff, 0xff, 0xff, 0xff, 0], &mut 0).is_err());
    }
}
