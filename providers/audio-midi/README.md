# audio-to-MIDI candidate provider

This optional local profile wraps Basic Pitch 0.4.0 with its explicit ONNX model
and CPU execution. aniflow keeps source audio immutable and records probabilistic
note candidates; it does not certify a score, instrument, voice, musical intent,
or transcription accuracy. Model activations are not calibrated confidence.

The bounded input is mono 22,050 Hz PCM16 RIFF/WAV of at most 120 seconds.
Unsupported source profiles receive a typed unavailable outcome. No resampling,
downmixing, dependency installation, model download or network inference occurs
implicitly. Supply every executable, adapter, runtime and model pin explicitly.

The Pipeline v3 capability has three immutable inputs (`audio`, `technical`,
`upstream_analysis`) and two immutable evidence outputs (`midi`, `analysis`).
`midi` is a JSON candidate report. MIDI export is a separate explicit operation
that emits a type-0 MIDI file and a companion loss/evidence report. An unavailable
analysis cannot be exported. The export clock is a serialization clock, not an
inferred musical tempo or meter; notes remain candidates.

- `configuration.schema.json` describes caller settings.
- `provider-configuration.schema.json` adds inspection tools and source binding.
- `manifest.json` identifies the exact raw provider-schema bytes.
- `pipeline.yml` is the reference stage shape; runtime registration binds the
  exact selected tool, model and implementation identities.
- `configuration.example.json` is a synthetic shape fixture. Its marker paths
  and digests are not a runnable installation or real-model validation evidence.

The Python adapter uses isolated mode and a bounded full installed-distribution
inventory. Those observations do not close over the operating system, Python
standard library, dynamic libraries or native hardware and do not establish an
operating-system sandbox. Provider effect declarations express requested
authority; they do not enforce filesystem or network confinement.

See [the user guide](../../docs/audio-midi.md) and the
[provider authoring contract](../../docs/provider-authoring.md). Run independent
schema checks with `python3 tests/audio_midi_schema.py` using an already installed
`jsonschema` package. The fixtures are synthetic and never download or run model
weights.
