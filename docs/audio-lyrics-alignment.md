# Reviewed-lyrics alignment candidates

`audio lyrics align` proposes source-relative word and cue timing for lyrics the
caller has already reviewed. It preserves the supplied text, cue identities and
review provenance. Candidate timing remains a model estimate requiring review;
it does not establish that a word occurs in the recording or that the timing is
correct. Source audio and reviewed-lyrics files remain unchanged.

This is [#49](https://github.com/egohygiene/aniflow/issues/49), checkpoint eight
of the open [#13 audio mini roadmap](https://github.com/egohygiene/aniflow/issues/13),
implemented in [draft PR #61](https://github.com/egohygiene/aniflow/pull/61).
It builds on technical inspection and the [timed-text contract](timed-text.md).
[Observed transcription](audio-transcription.md) is a separate operation;
a transcript never gains review authority by being passed to alignment.

## Prepare reviewed text

The lyrics input is an explicit `aniflow.timed-text/v1` JSON document whose
provenance is `reviewed_lyrics`. A supplied review authority and evidence identity
remain declarations; aniflow does not authenticate the reviewer or independently
verify a review occurred. Unreviewed text and observed-transcript documents are
refused instead of promoted automatically.

Use the existing [timed-text import](timed-text.md#cli-and-immutable-output-package)
with an explicit context to normalize lyrics you have reviewed, then retain the
normalized document and its original review evidence. Keep original punctuation,
whitespace and cue boundaries in that document. Alignment does not rewrite the
lyrics to match speech recognition output.

Two identities remain distinct: the normalized document's embedded `source`
identifies its original text, while the alignment input identity hashes the
exact reviewed JSON transport bytes. A changed file, text revision, cue order or
review declaration changes the bound input and cannot reuse an incompatible
checkpoint. Any supplied audio binding must agree with the selected inspected
source; declaring a binding is not proof of synchronization.

The input must declare language `en`. Tokenization admits ASCII letters with
internal apostrophes, using ASCII punctuation and whitespace as delimiters;
digits and non-ASCII text are refused by this bounded English profile. It maps
words to exact UTF-8 byte ranges within each original cue. The provider's normalized token spelling is separate from the
authored substring. Punctuation and spaces remain in the original text rather
than disappearing into the aligner's phrase. Missing and ambiguous matches remain explicit, while unsupported token syntax
is refused; no word is silently discarded to report a complete alignment. A complete candidate means the admitted tokens received structurally
valid timing, not that those timings were acoustically confirmed.

## Selected optional profile

The adapter targets [PocketSphinx 5.1.1](https://github.com/cmusphinx/pocketsphinx/releases/tag/v5.1.1)
at upstream revision `511126b492dcb267cf30d49d631946d7b61a9530`.
The operator supplies a trusted local executable, the English acoustic resources
and pronunciation dictionary with explicit byte identities. The executable
version is a supplied profile declaration; this integration does not observe a
version banner or prove that arbitrary matching bytes came from that release.
There is no automatic installation, model download, cache discovery, fallback
transcriber or network service.

| Input property | Supported profile |
| --- | --- |
| Audio | Nonempty mono PCM16 RIFF WAV admitted by technical inspection |
| Clock | Exactly 16,000 Hz with zero source-relative origin |
| Audio bounds | At most 600 seconds and 256 MiB |
| Text bounds | Reviewed JSON at most 64 KiB; at most 512 tokens and a 16 KiB alignment phrase |
| Text | Explicit reviewed-lyrics JSON and the bounded English token profile |
| Resources | Caller-pinned local PocketSphinx executable, seven acoustic files and dictionary |
| Processing | CPU forced alignment; no resampling, downmix, translation or source mutation |
| Scope | Whole supported mix or a full stem through the existing lineage importer |
| Confidence | No calibrated probability or quality score inferred from native output |

Forced alignment constrains a decoder to the supplied phrase. It can produce
plausible timestamps for words that are absent, mistyped or obscured by music.
The language profile and successful native execution therefore do not qualify
singing accuracy, repeated phrases, backing vocals, diarization or alignment to
a different recording. A token absent from the supplied pronunciation dictionary
can make native execution fail; it is not silently rewritten or assigned a
made-up pronunciation. Native timing is accepted only when it satisfies the
bounded parser and exact source-clock checks; the adapter does not clamp,
reorder or fabricate missing spans.

## Configure local dependencies

The authored settings use
[`aniflow.audio-alignment.configuration/v1`](../providers/audio-alignment/configuration.schema.json).
All dependency locations and pins are explicit:

| Field | Required value |
| --- | --- |
| `schema` | `aniflow.audio-alignment.configuration/v1` |
| `pocketsphinx` | Absolute `executable`, supplied `version` of `5.1.1`, exact `sha256` |
| `model` | Absolute `directory`, `model_id` of `en-us`, supplied `revision`, exact `files` inventory |
| `model.files` | Seven entries with `name`, `sha256` and `byte_size` |
| `dictionary` | Absolute `path`, exact `sha256` and `byte_size` |
| `language` | `en` for the admitted English profile |
| `tool_timeout_milliseconds` | Explicit subprocess deadline from 1 to 120,000 ms |
| `maximum_tool_output_bytes` | Explicit per-stream capture bound from 1,024 to 1,048,576 bytes |

The acoustic inventory names are `feat.params`, `mdef`, `means`, `noisedict`,
`sendump`, `transition_matrices` and `variances`, in that exact sorted order.
The acoustic files total at most 64 MiB; the dictionary is at most 16 MiB.
Each pin identifies bytes from
an independently prepared local installation. A hash match proves identity
against the authored configuration, not publisher authenticity, licensing rights
or an independently observed model revision. Unknown fields and unsupported
resource inventories are refused. The source/tool-bound
[provider wrapper](../providers/audio-alignment/provider-configuration.schema.json)
is constructed by the facade; operators author the settings document.

The executable is bounded to 128 MiB. Settings are bounded to 64 KiB; model
control files `feat.params` and `noisedict` must match the exact admitted upstream
profile because native feature configuration can override command arguments.
The library offers a 512 MiB / 24-file outer execution limit for private staging;
`AlignmentRequest::new` preserves explicit inspection limits, and the CLI uses
these defaults only when the caller omits the corresponding limits.

The adapter copies its admitted dependencies and selected audio into private
staging, uses direct arguments and a clean child environment, and rechecks byte
identities. The bounded working directory and declared side effects are not an
operating-system sandbox. Shared libraries and compiled-in native dependencies
remain part of the trusted installation; this profile does not claim a complete
native dependency closure.

The fixed native arguments use PocketSphinx's own single-dash parameter names.
The locations below are private runtime paths; they are not caller-authored
extra arguments:

```text
-hmm <private-model-directory>
-dict <private-dictionary>
-lm <private-directory>/disabled.lm
-samprate 16000
-frate 100
-phone_align no
-state_align no
-fsgusealtpron no
-loglevel ERROR
align <private-audio-snapshot> <normalized-reviewed-phrase>
```

The phrase is one direct argument. The deliberately nonexistent private
`disabled.lm` sentinel prevents the CLI from probing a compiled-in default
language model. The upstream `align` path clears the language-model setting
before decoder initialization, so no language-model file is created or loaded.
Phone/state timing and alternate-pronunciation output are disabled; the adapter
consumes only the strict basic word records.

The pinned [upstream CLI implementation](https://github.com/cmusphinx/pocketsphinx/blob/511126b492dcb267cf30d49d631946d7b61a9530/programs/pocketsphinx_main.c)
defines the forced-alignment invocation and native word records. The
[tool license](https://github.com/cmusphinx/pocketsphinx/blob/511126b492dcb267cf30d49d631946d7b61a9530/LICENSE)
contains the main BSD-2-Clause notice and separate bundled-component notices;
the [English acoustic model notice](https://github.com/cmusphinx/pocketsphinx/blob/511126b492dcb267cf30d49d631946d7b61a9530/model/en-us/en-us/README)
records Alpha Cephei's two-clause terms. Reports retain these source links and
CMU dictionary attribution as declarations, without authenticating arbitrary
supplied assets. Nothing in this profile vendors or downloads them.

## Plan, align and resume

Supply the existing explicit
[technical tool configuration](audio-inspection.md#configure-exact-local-tools),
a separate alignment configuration and reviewed lyrics:

```bash
aniflow --output json audio plan --analysis lyrics-alignment \
  --input "/absolute/synthetic/vocal.wav" \
  --configuration "/absolute/config/tools.json" \
  --alignment-configuration "/absolute/config/alignment.json" \
  --lyrics "/absolute/synthetic/reviewed-lyrics.json"

aniflow --output json audio lyrics align \
  --input "/absolute/synthetic/vocal.wav" \
  --configuration "/absolute/config/tools.json" \
  --alignment-configuration "/absolute/config/alignment.json" \
  --lyrics "/absolute/synthetic/reviewed-lyrics.json" \
  --output-directory "/absolute/generated-runs"

aniflow --output json status-v3 "/absolute/generated-runs/RUN_ID"

aniflow --output json audio resume "/absolute/generated-runs/RUN_ID" \
  --analysis lyrics-alignment \
  --input "/absolute/synthetic/vocal.wav" \
  --configuration "/absolute/config/tools.json" \
  --alignment-configuration "/absolute/config/alignment.json" \
  --lyrics "/absolute/synthetic/reviewed-lyrics.json"
```

Planning performs dependency preflight and binds source/text identities without
creating a run workspace or running alignment. A ready preflight proves neither
successful inference nor source-profile acceptance. `audio plan` and
`audio resume` retain their technical default; select `lyrics-alignment`
explicitly. The dedicated `audio lyrics align` command selects this family.

The [stem-selection flags](audio-stem-lineage.md) apply when selecting an
accepted full stem: keep `--input` bound to the original mix and supply its
accepted run, stage and artifact ID. Candidate timestamps belong to the selected
stem's clock. Retained lineage does not prove onset/phase alignment with the mix
and does not rerun the separator.

Equivalent Task wrappers are `audio:lyrics:plan`, `audio:lyrics:align` and
`audio:lyrics:resume`. Supply `INPUT`, `CONFIGURATION`,
`ALIGNMENT_CONFIGURATION` and `LYRICS`; execution also takes `OUTPUT`, and resume
takes `RUN`. `ANIFLOW_BIN` selects an explicitly built executable:

```bash
task audio:lyrics:align \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/synthetic/vocal.wav" \
  CONFIGURATION="/absolute/config/tools.json" \
  ALIGNMENT_CONFIGURATION="/absolute/config/alignment.json" \
  LYRICS="/absolute/synthetic/reviewed-lyrics.json" \
  OUTPUT="/absolute/generated-runs"
```

Task values are passed as quoted arguments. JSON successes use `audio_plan`,
`audio_lyrics_align` or `audio_resume` in the existing machine envelope. Failures
retain structured diagnostics on stderr with a nonzero exit status. Read-only
`status-v3` inspects persisted evidence; planning, execution and compatible
resume reobserve the configured dependencies and source/text inputs.

## Evidence and public API

The ordered workflow is `inspect_audio`, optional `attach_stem_lineage`, then
`analyze_alignment`. It retains the technical and optional lineage exports and
adds:

| Export | Run-relative path |
| --- | --- |
| `alignment` | `artifacts/audio-alignment/alignment.json` |
| `analysis` | `artifacts/audio-alignment/analysis.json`, artifact ID `alignment_analysis` |

The [alignment companion](contracts/audio-alignment-v1.schema.json) separates
immutable reviewed input from candidate timing, token mapping, result
availability and exact source/provider/configuration identities. Results are
`candidate` or `unavailable`; unsupported sample rates, unsupported channel counts
and exactly silent supported input have explicit unavailable reasons. Word
timings are `candidate`, `unmatched` or `ambiguous`. Cue timing additionally uses
`partial` when only part of its authored text has a usable mapping. Every cue is
retained in derived timed text; a cue without complete candidate timing remains
untimed. Candidate boundaries use exact reduced rational seconds on the native
10 ms grid.

The optional `raw_observation` identifies captured native JSON by digest and byte
size with artifact ID `alignment_observation`; it does not promise a persisted raw
artifact path. The embedded reviewed input retains its source text and supplied
review evidence independently of that native observation. The
[preflight contract](contracts/audio-alignment-preflight-v1.schema.json) records
dependency readiness and typed refusals. Rust validators add cross-field,
source-clock and derived-document checks beyond JSON Schema structure. Parsing
a supplied report alone does not prove its media bytes, reviewer identity or
acoustic accuracy.

The public `aniflow::audio_alignment` module exposes
`AlignmentRequest::new(inspection, configuration, lyrics)` using an existing
`AudioInspectionRequest` and validated `AlignmentConfiguration`. The `plan`,
`run` and `resume` functions use existing cancellation/progress and Pipeline v3
results. `preflight` checks alignment and technical-tool configuration without
processing media. Failures retain typed dependency or planning evidence for
callers to inspect.

Provider `org.egohygiene.aniflow.audio-alignment`, runtime capability
`aniflow/audio-lyrics-alignment` and implementation
`aniflow-audio-alignment-v1` identify this bounded integration. Its provider and
capability versions are `1.0.0`; the normalized audio-analysis foundation remains
unchanged. Provider-owned alignment evidence is an ordinary artifact, separate
from #33's general completion gates and #34's cross-run reuse.

## Export candidate timing

Export validates the report and projects its candidate timed-text document into
the existing loss-aware
[timed-text conversion package](timed-text.md#cli-and-immutable-output-package).
All authored cues remain present. Partial, unmatched and ambiguous cues stay
untimed: normalized JSON can preserve them, and plain text can export them with
any required explicit timing loss. An interval format such as WebVTT refuses
untimed cues; export never drops them or supplies guessed timestamps.
It preserves the supplied reviewed text authority; that authority does not
approve the newly estimated timing. Keep the original alignment report with the
export package so downstream review can inspect the estimate and retained input.
The library function `export_alignment_file` returns an `AlignmentExportOutcome`
with `timing_authority: "candidate"` and the original alignment-report identity.
Its nested conversion input identifies the extracted canonical timed-text JSON,
not the enclosing report. The report is rechecked before the completion marker
is published. A standalone subtitle carrier is not a timing-review
attestation.

```bash
aniflow --output json audio lyrics export \
  --alignment "/absolute/generated-runs/RUN_ID/artifacts/audio-alignment/alignment.json" \
  --to webvtt --allow-loss language,metadata \
  --output-directory "/absolute/generated-evidence/alignment-webvtt"
```

Derived timed text carries `aniflow_alignment_timing: "candidate"` in its
metadata. Every registered text carrier requires explicit `metadata` loss
permission for that marker; the normalized input companion keeps it. WebVTT also
requires `language` loss permission for the English declaration. Additional
permissions depend on other input fields and the selected carrier. Loss
permissions do not authorize dropping unmatched words, changing reviewed text
or filling missing timing. The destination must be new,
and the conversion report is published last as its completion marker. JSON
success uses `audio_lyrics_export`; the Task equivalent is
`audio:lyrics:export` with `ALIGNMENT`, `TO`, `OUTPUT` and optional `ALLOW_LOSS`.

## Local evidence and remaining gates

Focused checks use synthetic fixtures only:

```bash
task audio:lyrics:corpus
task audio:lyrics:schema
```

The [checkpoint record](validation/aniflow-49-checkpoint.md) records recoverable
savepoints and actual local validation outcomes. Synthetic protocol, refusal,
resume and export fixtures can establish contract behavior; they do not qualify
real PocketSphinx execution or alignment accuracy. Native macOS, broader input
profiles, hosted CI and release qualification require their own evidence. No
real media or model weights are required by these fixtures.
