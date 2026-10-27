# Import an existing frame workspace

Import a complete PNG workspace into a new isolated run before using it for
later selective repairs. The import binds the original source file's observed
SHA-256 to an existing `aniflow.rife-frame-index/v1`, preserves the declared
frame order and rational clock, and copies every admitted PNG without changing
its bytes. It uses the existing Pipeline v3 provider, artifact and checkpoint
owners.

This is an authored [#36](https://github.com/egohygiene/aniflow/issues/36)
checkpoint. Tests, builds, schema checks, native execution and qualification
remain unrun under [#64](https://github.com/egohygiene/aniflow/issues/64).

## Plan, then import

Keep the JSON index outside the PNG directory: directory membership must match
the index exactly. Supply a source video, its existing complete frame index,
the PNG directory, and [decoder settings](../providers/frame-import/configuration.schema.json).

```bash
aniflow --output json video import-frames \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/frames" \
  --frame-index "/absolute/frame-index.json" \
  --configuration "/absolute/frame-import.json" \
  --plan
```

`--plan` returns the existing `aniflow.pipeline-plan/v1` shape. It resolves
identities and declarations, checks actual source/frame bytes against their
declared digests and applies the configured byte/pixel bounds without launching
FFmpeg or creating an output workspace. A plan is not a successful decode or
accepted import. An output directory supplied with `--plan` is unused.

After reviewing the plan, omit `--plan` and select the parent for a newly named
isolated run:

```bash
aniflow --output json video import-frames \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/frames" \
  --frame-index "/absolute/frame-index.json" \
  --configuration "/absolute/frame-import.json" \
  --output-directory "/absolute/import-runs"
```

Both forms use machine command `video_import_frames`. Successful execution
returns the existing `aniflow.pipeline-run-outcome/v1` with the run directory
and output references; it does not replace that result with a new run format.
Machine success stays on stdout and machine failure stays on stderr. Human
progress remains presentation, separate from durable acceptance evidence.

The public `frame_import` library module owns the same behavior. The CLI
supplies typed paths/settings and uses the current aniflow executable as its
native provider. Use `frame_import::run` or the CLI for ordinary execution;
`execute_invocation` is the native ABI implementation and does not itself create
an accepted checkpoint or supply the outer process-group cancellation owner. It does not introduce another runner or checkpoint format.
Input paths must resolve to canonical nonsymlink paths. Relative CLI input
paths are made absolute but are not silently canonicalized through symlinks or
parent traversal. Keep the output parent outside every input artifact; the facade checks this
before creating a run, including when the library default parent is used.
Nonsymlink checks and re-observation do not promise atomic directory-handle
confinement against hostile concurrent ancestor replacement.

## Pin the decoder and bound the work

The configuration requires `schema: "aniflow.frame-import-configuration/v1"`
and `ffmpeg` with its absolute normalized executable path, exact semantic
version and SHA-256. Obtain those values from the tool installation you intend
to trust; do not copy placeholder digests. No tool download, installation,
network access, backend fallback or PATH discovery is implicit.

| Setting | Default | Allowed maximum |
| --- | ---: | ---: |
| `tool_timeout_milliseconds` | 120,000 ms per invocation | 120,000 ms |
| `maximum_source_bytes` | 64 GiB | 1 TiB |
| `maximum_frame_bytes` | 64 MiB | 64 MiB |
| `maximum_total_frame_bytes` | 64 GiB | 1 TiB |
| `maximum_pixels` | 8,400,000 pixels per image | 8,400,000 |

The decoder executable is bounded to 128 MiB. Each child stdout/stderr capture
is bounded to 64 KiB. The outer import invocation defaults to six hours, with
1 MiB stdout/stderr capture, 100,002 artifact files, and 64 GiB plus 64 MiB
artifact bytes. These outer bounds remain independent of caller configuration;
raising a configuration limit does not silently raise them. Library callers
can explicitly supply their own existing `ProviderExecutionLimits`.

The source bound starts at one byte, the frame bound at 1 KiB, the total-frame
bound at 1 MiB, and the pixel bound at one. The existing index limit is 16 MiB
and 100,000 frames; the complete import report is limited to 64 MiB. A valid
frame count can still exceed the evidence byte budget. Defaults are materialized
before creating exact effective provider configuration; omitted defaults in a
raw provider invocation do not bypass that identity check.

Each frame is decoded serially. FFmpeg's
[framehash muxer](https://ffmpeg.org/ffmpeg-formats.html#framehash) emits the hash
and byte count of the decoded raw RGB24 packet, so the importer need not retain
an entire uncompressed frame in its own output capture. It requires exactly one
packet with `width × height × 3` bytes. The strict PNG profile rejects animation;
no frame-limit option hides extra decoded output. It explicitly selects format
version 2 and refuses side-data extensions rather than interpreting them as
the admitted six-column packet shape. The pinned primary-source basis is
FFmpeg n8.0 commit `140fd653aed8cad774f991ba083e2d01e86420c7`:
[`hashenc.c`](https://github.com/FFmpeg/FFmpeg/blob/140fd653aed8cad774f991ba083e2d01e86420c7/libavformat/hashenc.c)
defines version/packet output, and
[`framehash.c`](https://github.com/FFmpeg/FFmpeg/blob/140fd653aed8cad774f991ba083e2d01e86420c7/libavformat/framehash.c)
defines media, codec and geometry headers. That source inspection does not
qualify a local FFmpeg build.

These limits bound admission, time, captured tool output and observed artifacts.
They do not reserve filesystem space, enforce process RSS, isolate the decoder
in an OS sandbox or establish native safety. The pinned executable remains
trusted code. Test qualification on the intended host is still required.

## What the import observes

The frame index must describe the complete source timeline. The importer checks
the same one-to-one ordered filenames, zero-based ordinals, source geometry,
exact rational timing, cut boundaries and supported zero-origin CFR profile as
the [frame-selection preview](frame-selection.md). It also reads actual files:
the PNG directory must contain exactly the indexed regular files, with no
missing, extra, nested or special entries. Every file must match its declared
digest.

The admitted image profile is noninterlaced RGB8 PNG, with no alpha and only
the supported SDR/square-pixel declarations. Structural inspection checks PNG
chunks and CRCs. A pinned local FFmpeg then fully decodes each private snapshot;
the import requires one complete RGB24 frame with the declared dimensions and
exact decoded byte count. Structural checks alone are not successful decode
evidence. The stored PNG bytes remain unchanged; decoder output is observation,
not a replacement image.

The source byte count must also match the temporal inspection and be nonzero.
Source and input identities are checked again before a successful provider
return. The ordinary runtime separately observes outputs and applies its
required artifact-integrity gates before publishing a stage checkpoint. Process
exit, provider observations and runtime acceptance remain separate facts.
Interrupted or invalid candidate outputs are retained by the existing run
runtime; no accepted checkpoint is manufactured from a partial directory.
The dedicated command starts a new import. It does not add a separate partial
frame-resume protocol.

The fixed `aniflow/frame-import` capability has these immutable ports:

| Port | Direction | Artifact |
| --- | --- | --- |
| `source` | Input | Original source video bytes |
| `frames` | Input | Complete indexed PNG directory |
| `index` | Input | Existing `aniflow.rife-frame-index/v1` document |
| `frames` | Output | Byte-preserved PNG directory |
| `evidence` | Output | `aniflow.frame-import-report/v1` |

The [pipeline](../providers/frame-import/pipeline.yml) declares both output
paths and their mandatory integrity gates. The
[report contract](contracts/frame-import-report-v1.schema.json) retains the
complete index, source and configuration digests, current provider-lock digest,
decoder identity, and original/decoded identities for every frame. Its
`source_relationship` is `declared_not_verified` and `historical_processing`
is `unknown`.

## What the evidence means

The source digest identifies the supplied source bytes. The timing map and
frame-to-source relationship are caller declarations bound to those bytes; this
import does not independently extract the video again or prove that a PNG was
derived from a particular video frame. A valid decode also does not establish
visual quality, authentic colors, authorship or freedom from earlier edits.

Historical frame processing remains **unknown**. The new import has its own
real contemporary provider execution, exact tool/configuration identity and
runtime evidence. Those records describe the import only: no prior provider
lock, model identity, repair attempt or original extraction is fabricated from
image dimensions or filenames.

An accepted import is an immutable intermediate frame stage, not a validated
final video or a released/native-qualified capability. It does not repair
selected frames, assemble video, remux audio, infer scene cuts, normalize VFR,
remove alpha, rotate, resize, tone-map or alter the originals.

## Remaining workflow

The [read-only selection command](frame-selection.md) remains useful for
previewing inclusive user ranges. Selective repair execution still needs an
explicit provider/profile, separate edited outputs, exact returned cardinality
and timing, and byte-for-byte preservation of every unselected frame. Clip-only
provider limits and seam handling remain explicit follow-up work; truncating
299 requested frames to 240 is never a valid repair.

The standalone `video-frames/1` schema/example is not pinned in this repository.
This checkpoint accepts the existing aniflow frame index only and makes no
migration or compatibility promise for a guessed external format.

See the [checkpoint handoff](validation/aniflow-36-import-checkpoint.md) for
source-review evidence, unexecuted checks and continuation boundaries.
