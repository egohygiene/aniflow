# Bounded offline transcription

`audio transcribe` produces observed speech or lyric text with source-relative
segment timing through an optional local provider. It uses the existing
technical inspector, source/stem identity and Pipeline v3 execution/checkpoints.
The source is never replaced. Transcription is an observation from a model, not
reviewed or canonical lyrics.

This is [#48](https://github.com/egohygiene/aniflow/issues/48), checkpoint seven
of the open [#13 audio mini roadmap](https://github.com/egohygiene/aniflow/issues/13),
under review in [PR #60](https://github.com/egohygiene/aniflow/pull/60).
The normalized `aniflow.audio-analysis/v1` contract remains unchanged.
[Reviewed-lyrics alignment](https://github.com/egohygiene/aniflow/issues/49) is a
separate feature; a successful transcription never grants review authority.

## Selected optional profile

The selected adapter targets [whisper.cpp `1.8.7`](https://github.com/ggml-org/whisper.cpp/releases/tag/v1.8.7),
source revision `48f628a84833905ee4a0658ee6d4a5c915ce1997`, an explicitly supplied local
CPU executable and the English `tiny.en` f16 model. The operator prepares those
dependencies separately and authors their exact identities. Planning, execution
and resume do not invoke an installer, fetch weights, discover a model cache or
fall back to a network transcription service.

| Input property | Supported profile |
| --- | --- |
| Container and samples | Nonempty PCM16 RIFF WAV within the existing technical-inspection limits |
| Sample clock | Exactly 16,000 Hz, zero source-relative origin, one audio stream |
| Channels | Mono only |
| Bounds | At most 600 seconds and 256 MiB |
| Preprocessing | No resampling, downmix, ReplayGain or source mutation |
| Scope | Whole supported mix or a full stem through the existing lineage importer |

An otherwise valid technical source with another rate or channel count retains
its inspection evidence and reports transcription as unavailable. The workflow
does not silently prepare a different audio clock. In particular, a stereo
source is not implicitly averaged into mono.

The profile consumes the pinned CLI's basic JSON segment output. It retains
exact tool/model/configuration identities and the captured raw JSON digest;
downstream consumers use the normalized report instead of parsing tool-specific
output or relying on a private raw-output path.

| Evidence | Meaning |
| --- | --- |
| Text | Observed transcript, with probabilistic provenance |
| Language | The selected English profile; not multilingual language detection |
| Segment timing | Supplied source-relative segment boundaries, checked against the selected source clock |
| Word timing | Explicitly unavailable in this profile |
| Segment confidence | Explicitly unavailable; no invented score or calibrated probability |
| Empty observation | Retained as an honest empty result; no placeholder timed-text cue |
| Review authority | Never inferred from model output, validation or export |

Timestamp bounds and contract validation can reject malformed or out-of-range
segments. They cannot establish that the recognized words are correct, that
speech is present or that a model has not hallucinated plausible text within
valid time bounds. Broader language support and richer timing granularity
require separately qualified profiles.

Segment boundaries use reduced rational seconds on the native 10 ms grid.
They must be nonempty, ordered, disjoint and within the exact source duration;
the adapter never clamps an out-of-range span into validity. Translation, VAD,
word timestamps, GPU inference and quantized/multilingual model variants are
outside this profile. An empty native segment array means only that this
invocation observed no segments, not that the audio contains no speech.

## Configure prepared local dependencies

The settings schema is
[`aniflow.audio-transcription.configuration/v1`](../providers/audio-transcription/configuration.schema.json).
Every field is explicit; no model location, language or execution budget is
silently discovered. This example uses the selected reference model metadata and
an intentionally unusable executable digest:

```json
{
  "schema": "aniflow.audio-transcription.configuration/v1",
  "whisper": {
    "executable": "/absolute/prepared/whisper-cli",
    "version": "1.8.7",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  },
  "model": {
    "path": "/absolute/prepared/ggml-tiny.en.bin",
    "sha256": "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
    "byte_size": 77704715,
    "model_id": "tiny.en",
    "revision": "5359861c739e955e79d9a303bcbc70fb988958b1"
  },
  "language": "en",
  "threads": 4,
  "tool_timeout_milliseconds": 120000,
  "maximum_tool_output_bytes": 1048576
}
```

Replace the executable path/digest and model path with your explicitly prepared,
trusted regular files. The fixed tool version banner and the actual bytes must
match the authored pins. The model revision, size and SHA-256 above come from
the selected repository's
[file metadata](https://huggingface.co/api/models/ggerganov/whisper.cpp/tree/5359861c739e955e79d9a303bcbc70fb988958b1?recursive=false&expand=false);
documenting that metadata does not download, inspect or execute the weights.
The runtime retains the caller's model revision as supplied provenance; a hash
match does not independently prove that revision or publisher.

The selected [tool license](https://github.com/ggml-org/whisper.cpp/blob/48f628a84833905ee4a0658ee6d4a5c915ce1997/LICENSE)
and [model card](https://huggingface.co/ggerganov/whisper.cpp/blob/5359861c739e955e79d9a303bcbc70fb988958b1/README.md)
declare MIT. The report preserves these expressions and source URLs as licensing
evidence, not a legal conclusion or authentication of arbitrary supplied files.
Prepare a CPU-only build separately following the pinned upstream source;
runtime execution never builds or installs it.

Settings are bounded to 64 KiB and reject unknown fields. Paths must be absolute,
normalized UTF-8 paths; final symlinks are refused. The executable is bounded to
128 MiB and the model to 256 MiB. Threads range from 1 to 16, each subprocess
deadline from 1 to 120,000 ms, and each stdout/stderr capture from 1,024 to
1,048,576 bytes. These ceilings are refusal boundaries, not a promise that the
model finishes within the chosen budget on every CPU.

The facade constructs the separate
[provider configuration](../providers/audio-transcription/provider-configuration.schema.json)
to bind settings to inspection tools, source identity and upstream analysis.
Operators author the settings document, not that internal wrapper. Local paths
remain operational locators; public settings/command evidence uses identities
and symbolic placeholders.

## Dependency identity and execution boundary

The configured tool and model hashes are reobserved before execution and
compatible run-local resume. They establish byte identity against the supplied
pins, not download origin, signature verification or build authenticity.
The adapter stages the executable and model into a private empty working
directory and uses a clean child environment for the fixed CPU profile. This
limits ambient dependency selection; it is not an operating-system sandbox.
Compiled-in backend paths and dynamically linked dependencies remain part of the
trusted installation, not a fully verified native dependency closure.

The existing provider runtime owns bounded direct argument invocation,
cancellation, timeouts, capture limits and output acceptance. Missing/stale
dependencies, cancellation, malformed output or failed execution cannot become
a complete transcript. Ordinary provider evidence does not become the general
layered completion gate from #33, and run-local reuse does not implement #34's
cross-run cache.

Read-only `status-v3` inspects persisted run evidence without probing the current
tool/model installation. Planning, execution and resume reobserve dependencies.
A binary requiring adjacent shared libraries is unavailable when isolated staging
cannot load it; this profile does not silently copy an unbounded dependency tree.

## Plan, transcribe and resume

Supply the existing explicit
[technical tool configuration](audio-inspection.md#configure-exact-local-tools)
and a separate transcription configuration:

```bash
aniflow --output json audio plan --analysis transcription \
  --input "/absolute/synthetic/speech.wav" \
  --configuration "/absolute/config/tools.json" \
  --transcription-configuration "/absolute/config/transcription.json"

aniflow --output json audio transcribe \
  --input "/absolute/synthetic/speech.wav" \
  --configuration "/absolute/config/tools.json" \
  --transcription-configuration "/absolute/config/transcription.json" \
  --output-directory "/absolute/generated-runs"

aniflow --output json status-v3 "/absolute/generated-runs/RUN_ID"

aniflow --output json audio resume "/absolute/generated-runs/RUN_ID" \
  --analysis transcription \
  --input "/absolute/synthetic/speech.wav" \
  --configuration "/absolute/config/tools.json" \
  --transcription-configuration "/absolute/config/transcription.json"
```

Planning includes dependency preflight without creating a run workspace. It does
not establish successful model inference or recognition quality. `audio plan`
and `audio resume` keep their technical default, so select transcription
explicitly. The dedicated `audio transcribe` command selects this family.

The corresponding task shortcuts are `audio:transcription:plan`,
`audio:transcribe` and `audio:transcription:resume`. Supply `INPUT`,
`CONFIGURATION` and `TRANSCRIPTION_CONFIGURATION`; execution also takes `OUTPUT`
and resume takes `RUN`. `ANIFLOW_BIN` selects an explicitly built executable:

```bash
task audio:transcribe \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/synthetic/speech.wav" \
  CONFIGURATION="/absolute/config/tools.json" \
  TRANSCRIPTION_CONFIGURATION="/absolute/config/transcription.json" \
  OUTPUT="/absolute/generated-runs"
```

The existing [stem-selection flags](audio-stem-lineage.md) apply when a validated
full stem is supplied. Keep `--input` bound to the original mix and select the
accepted run, stage and artifact ID. Transcript times belong to the selected
stem's clock; the lineage import does not prove onset/phase alignment with the
mix and does not rerun the separator.

Successful JSON envelopes use `audio_plan`, `audio_transcribe` or `audio_resume`
on stdout. Structured failures use stderr and a nonzero exit; dependency
preflight and existing Pipeline v3 recovery evidence stay available to callers.

## Public artifacts and library boundary

The ordered workflow is `inspect_audio`, `attach_stem_lineage` when selected,
then `analyze_transcription`. It retains upstream evidence and exports:

| Export | Run-relative path and meaning |
| --- | --- |
| `technical` | `artifacts/audio-inspection/technical.json`; unchanged technical/decode evidence |
| `transcription` | `artifacts/audio-transcription/transcription.json`; normalized observed-transcription companion |
| `analysis` | `artifacts/audio-transcription/analysis.json`; final normalized envelope, artifact ID `transcription_analysis` |
| `stem_lineage`, when selected | `artifacts/audio-stem/lineage.json`; retained imported relationship evidence |

The companion is
[`aniflow.audio-transcription/v1`](contracts/audio-transcription-v1.schema.json).
It distinguishes `observed`, `empty` and `unavailable` results. Unsupported rate,
unsupported channels and exactly silent supported PCM have explicit unavailable
outcomes; only the two analyzer-observation states retain a raw-capture identity.
The optional embedded timed-text document exists only for nonempty observations
and must reproduce their text, segment boundaries and observed producer exactly.
It carries no supplied-reviewed authority.

`raw_observation` identifies captured basic JSON by digest and size. Those bytes
are not a persisted output artifact or a promise that a local path is available.
The embedded timed-text `source_text` identifies that capture, while its
`audio_source` separately binds the original inspected audio. Model, tool and
configuration identities do not turn the capture into authenticated ground truth.

[`aniflow.audio-transcription-preflight/v1`](contracts/audio-transcription-preflight-v1.schema.json)
records readiness and typed dependency refusals. A ready result means the
configured dependency checks passed; it proves neither source-profile acceptance
nor a successful transcription. The public `AudioTranscriptionReport` and
`AudioTranscriptionPreflight` parsers expose `from_json_slice` and semantic
`validate` methods. Schema validation alone does not authenticate supplied
evidence or establish recognition accuracy.

The public `aniflow::audio_transcription` module exposes
`TranscriptionRequest::new(inspection, configuration)` using an existing
`AudioInspectionRequest` and validated `TranscriptionConfiguration`. Its `plan`,
`run` and `resume` methods retain the existing cancellation token, progress
callbacks and Pipeline v3 results; `AudioTranscriptionFailure` retains typed
preflight/planning evidence. `preflight` accepts transcription settings,
inspection-tool settings and a cancellation token without processing audio.

Provider `org.egohygiene.aniflow.audio-transcription`, runtime capability
`aniflow/audio-transcription` and the normalized family use `1.0.0`;
implementation identity is `aniflow-audio-transcription-v1`. The provider lock's
`tiny.en` component version `1.0.0` identifies this integration profile, while
the supplied model revision/digest remain separate evidence. It is not a claim
that the upstream model has a `1.0.0` release.

Reports are bounded to 8 MiB and preflight evidence to 64 KiB. Raw JSON is at most
1 MiB. At most 10,000 segments may each contain 64 KiB of text, with at most 1 MiB
aggregate text; bounds refuse excessive output rather than truncate it.
`default_execution_limits` raises the outer transient artifact-tree budget to
704 MiB for bounded tool/model/source staging. `TranscriptionRequest::new`
preserves the caller's inspection limits, so library callers select that budget
explicitly when appropriate. The CLI uses the transcription default. Other
outer defaults remain 600 seconds per provider, 2 seconds termination grace,
1 MiB each stdout/stderr and 8 artifact files. Each stage is independently
bounded; the configured child deadline remains at most 120 seconds.

## Timed-text export

A nonempty accepted transcript can retain an observed `TimedTextDocument`.
Export uses the [existing loss-aware conversion utility](timed-text.md), including
its strict registered format subsets, immutable input, named loss permissions,
normalized companions and completion report. It does not invent reviewed lyrics
or fill unavailable word boundaries.

```bash
aniflow --output json audio transcript-export \
  --transcription "/absolute/generated-runs/RUN_ID/artifacts/audio-transcription/transcription.json" \
  --to webvtt \
  --allow-loss language \
  --output-directory "/absolute/generated-evidence/transcript-webvtt"
```

An empty observation has no fabricated cue to export. Export refuses when the
report has no convertible timed-text document. The caller may explicitly allow
known target losses with comma-separated or repeated `--allow-loss` values.
That permission cannot manufacture timing, confidence or review authority.
The WebVTT example explicitly permits loss of the English language field because
the registered WebVTT subset does not carry document language. The normalized
input companion still retains it.

The public `export_transcript_file` API validates the supplied report and rechecks
its bytes before publishing the conversion completion marker. It returns
`TranscriptExportOutcome`: the original `transcription_report` digest/size plus
the existing `FileConversionOutcome`. The nested conversion report's `input`
identifies the extracted canonical timed-text JSON, not the whole transcription
report. Export does not rerun a model. JSON CLI success uses
`audio_transcript_export` in the existing envelope.
The source report may be up to 8 MiB, but the extracted canonical timed-text JSON
and rendered payload each remain subject to the converter's 1 MiB limit. A valid
report can therefore be too large to export through this bounded carrier path.

Use `task audio:transcript:export` with `TRANSCRIPTION`, `TO`, `OUTPUT` and an
optional comma-separated `ALLOW_LOSS`; `ANIFLOW_BIN` selects the executable.

## Evidence and remaining qualification

Synthetic provider fixtures exercise dependency drift, output normalization,
timing bounds, unavailable granularity, empty results and refusal/recovery
behavior. They do not execute or qualify the selected speech model. Actual
whisper.cpp model inference and transcription accuracy remain unverified in this
environment; synthetic checks do not change that status. No real media is used
for fixture checks.

The [recovery checkpoint](validation/aniflow-48-checkpoint.md) records completed
checks and remaining work. Native-platform support, broader language/media
profiles, native dependency closure, hosted CI and release qualification remain
separate gates. The maintainer reviews and merges the checkpoint; parent #13
remains open.

Focused synthetic checks use `task audio:transcription:corpus` and
`task audio:transcription:schema`. The latter requires an already installed
Python `jsonschema` package. Synthetic shape examples and protocol fixtures are
not claimed real-model transcripts; the selected optional dependencies are never
installed by these tasks.
