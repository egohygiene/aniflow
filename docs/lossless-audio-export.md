# Export a selected lossless audio candidate

`lossless_audio` and `delivery audio-export plan|run|resume|status` produce one
explicitly selected WAV or FLAC candidate from current original PCM signal
evidence. They reuse one ordinary v3 stage, pinned tools, snapshots, cancellation,
checkpoints and recovery. No source mix is modified, normalized, trimmed,
resampled, dithered, requantized or downmixed. Creative review remains separate.

The first profile admits existing native PCM16/PCM24/float32 WAV sources within
the current 256 MiB, ten-minute, mono/stereo bounds. WAV is rebuilt as minimal
RIFF32 around unchanged native samples, including the required float fact count;
no codec default silently chooses another precision/header profile. FLAC admits
integer PCM16/PCM24 only. Float-to-FLAC refuses before work because it would need
a separate explicit precision policy. Select one format per run, with no fallback.

Use closed `aniflow.lossless-audio-configuration/v1` JSON:

```json
{
  "schema": "aniflow.lossless-audio-configuration/v1",
  "format": "flac",
  "tools": {
    "schema": "aniflow.audio-inspection.configuration/v1",
    "ffmpeg": {"executable": "/absolute/ffmpeg", "version": "8.0.1", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"},
    "ffprobe": {"executable": "/absolute/ffprobe", "version": "8.0.1", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"},
    "tool_timeout_milliseconds": 120000,
    "maximum_tool_output_bytes": 1048576
  },
  "metadata_policy": "remove_inherited_descriptive_metadata",
  "maximum_output_bytes": 268435456,
  "maximum_working_bytes": 4294967296,
  "minimum_free_bytes": 268435456
}
```

The zero hashes/paths are placeholders. Supply actual local pins deliberately;
nothing installs, downloads or invokes a tool during planning. File identities
are inspected by plan; native versions are observed only inside execution.
Each executable is bounded to the existing 128 MiB native-tool snapshot profile.
The explicit metadata policy removes inherited descriptive tags while preserving
the original master/history separately. Regenerated encoder/vendor and structural
seek/padding fields are excluded; no signature-validity, provenance-removal or
signal-mark absence is asserted. Public fields are supplied later for a package.

```console
aniflow --output json delivery audio-export plan \
  --source "/absolute/original mix.wav" --signal-run-directory "/absolute/accepted signal" \
  --configuration lossless.json
aniflow --output json delivery audio-export run \
  --source "/absolute/original mix.wav" --signal-run-directory "/absolute/accepted signal" \
  --configuration lossless.json --output-directory "/absolute/separate run parent"
aniflow --output json delivery audio-export resume \
  --source "/absolute/original mix.wav" --signal-run-directory "/absolute/accepted signal" \
  --configuration lossless.json --run-directory "/absolute/exact prior run"
aniflow --output json delivery audio-export status --run-directory "/absolute/exact prior run"
```

The initial executor requires Unix, an existing canonical parent separate from
source/evidence/executables, finite storage admission and exact source/plan/tool
bindings. Ordinary v3 creates the run root. Resume uses existing checkpoints and
refuses changed source, signal evidence, provider, tools or configuration. Loose
exports never become accepted runs. Use ordinary v3 status/recovery for unfinished
work; audio-export status returns only current complete acceptance. Failed private
work stays available without automatic cleanup/adoption.

The worker independently inspects WAV fields/PCM or FLAC STREAMINFO/metadata,
checks exactly one audio stream through FFprobe and compares a full decoded
native-PCM SHA-256 with the original native samples. FLAC encoding/decoding use
the same pinned FFmpeg implementation; this is not independent-codec or native-
platform qualification. Header parsing alone cannot establish full decode.
Actual source/candidate/tool checks precede ordinary v3 acceptance.

The private report retains original accepted signal measurements and exact
PCM/clock identity. Its after-export meter is explicitly `not_remeasured`:
unchanged PCM can reference original measurements, but no fresh loudness/true-
peak run is invented. Unavailable measurements stay unavailable. Fixed native
processing evidence, file integrity and creative approval remain distinct.

For [destination preflight](destination-preflight.md), select:

```json
{"kind": "accepted_audio_export", "run_directory": "/absolute/exact prior run"}
```

The current native owner rechecks complete v3 authority, original signal evidence
and retained file/report/container bindings without rerunning codecs. Supplied
dated PCM requirements evaluate decoded precision, channels, rate, complete
duration and actual output size. [Reviewed packages](posting-package.md) copy the
actual `audio.wav` or `audio.flac`; [release evidence](release-evidence.md) can
retain these final bytes. Metadata-scope acknowledgment and supplied exact package
review remain explicit. No posting or platform acceptance is implied.

Storage/free-space checks are admission observations, not reservations, kernel
quotas or host-RSS guarantees. Existing process deadlines/artifact limits retain
their scope. Additional encoders/transforms, fresh after-metering and full #83
quality/platform criteria stay open. Six synthetic header/argument/refusal cases
plus two CLI cases are authored, uncompiled/unrun. All execution, builds, checks
and CI remain deferred to [flow #88](https://github.com/egohygiene/flow/issues/88),
scheduled last.
