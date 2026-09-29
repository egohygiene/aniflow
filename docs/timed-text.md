# Lyrics and timed-text conversion

The [#47 checkpoint](https://github.com/egohygiene/aniflow/issues/47) provides
bounded, loss-aware text interchange for the
[#13 audio roadmap](https://github.com/egohygiene/aniflow/issues/13). It converts
explicitly selected text formats without reading audio, running a model or
guessing lyric timing. Source files remain unchanged.

The public `aniflow::timed_text` module owns the contracts and conversion rules;
the `timed-text` CLI uses the same library boundary. This utility does not need
an audio provider, FFmpeg, Python, a Pipeline v3 workspace or a checkpoint store.
Transcription and reviewed-lyrics alignment remain separate checkpoints.

## CLI and immutable output package

List profiles, then convert a supplied interval file into a new directory:

```bash
aniflow --output json timed-text formats
aniflow --output json timed-text convert \
  --input "/absolute/synthetic/captions.srt" --from srt --to webvtt \
  --output-directory "/absolute/generated-evidence/captions-webvtt"
```

Use `--context context.json` to supply import provenance, language, overlap policy
or an optional audio-source declaration. Context is explicit input, not a sidecar
discovered by filename. For normalized JSON input, omitting context preserves
the embedded context; an explicitly supplied context must match its provenance,
language, audio binding and overlap policy exactly. It cannot reset reviewed
lyrics to an unreviewed default. `--allow-loss` takes a comma-separated list and may be
repeated. A loss permission applies only to the named kind. For example, an
interval-to-LRC conversion can explicitly discard ends using
`--allow-loss end_times`; it may require additional permissions for other fields
the specific source carries.

The output directory must be new. After validating the conversion, the converter
creates that directory exclusively and creates each payload/companion file
without replacing existing files. It preserves the input file. The package
contains:

| File | Meaning |
| --- | --- |
| `payload.<extension>` | Converted carrier bytes, with the target format's extension |
| `normalized-input.json` | Validated source document, with original byte identity |
| `normalized-output.json` | Target document retaining provenance/binding and explicitly adjusted semantic fields |
| `conversion.json` | Input/output byte identities, normalized-document digests, losses, lexical facts and carrier omissions |

Normalized-document digests cover canonical compact UTF-8 JSON bytes with sorted
object keys and no trailing newline, exactly as written in the package. Published
JSON examples may be pretty-printed for reading; hashing their display formatting
does not reproduce a normalized-document digest.

The conversion report is the completion marker and is published atomically last.
The directory as a whole is not published by an atomic rename. An incomplete
directory is never reported as success; controlled failure removes only files
owned by the current attempt. An existing destination is refused, and the
command does not overwrite media or a prior conversion package. Keep the whole
package when review authority or audio binding matters.

Input paths and the existing destination parent must resolve without symlinks;
dot traversal is refused. Relative paths resolve from the working directory.
Use a directory controlled by the caller: these ownership checks are not a
sandbox against hostile concurrent replacement of filesystem paths. Consumers
must validate the completion report and artifact digests before accepting a
package.

When importing normalized JSON, the embedded `source` remains the original
text identity. It is not replaced with the JSON file's digest. The conversion
report's `input` and `output` identify the current transport bytes; its two
document hashes identify the exact normalized companions.

JSON successes use the `timed_text_formats` or `timed_text_convert` command in the
existing machine envelope on stdout. Failures use the corresponding structured
failure envelope on stderr and a nonzero exit status. No conversion failure is
silently promoted to a successful, lossy output.

Equivalent shortcuts are `task timed-text:formats` and:

```bash
task timed-text:convert \
  INPUT="/absolute/synthetic/captions.srt" FROM=srt TO=webvtt \
  OUTPUT="/absolute/generated-evidence/captions-webvtt"
```

Set `ANIFLOW_BIN` to select a built executable. The conversion task also accepts
`CONTEXT` and comma-separated `ALLOW_LOSS`; their values are passed as single
quoted arguments rather than executed as command text.

## Registered subsets

Format names are explicit inputs, not guesses from filenames. `plain`, `lrc`,
`srt`, `webvtt` and `ttml` are text carriers; `json` is the normalized document
transport. The extensions below describe emitted files, not automatic format
detection.

| Format | Extension | Accepted timing and text |
| --- | --- | --- |
| `plain` | `.txt` | One untimed whole-text cue, including Unicode and newlines |
| `lrc` | `.lrc` | One `[mm:ss.cc]` start time per single-line cue; no implied end |
| `srt` | `.srt` | Numeric source labels and explicit `HH:MM:SS,mmm --> HH:MM:SS,mmm` intervals; multiline plain text |
| `webvtt` | `.vtt` | Explicit millisecond intervals, optional cue labels and a whole-cue speaker wrapper |
| `ttml` | `.ttml` | Explicit media-time intervals in flat `tt/body/div/p` content with preserved whitespace and `br` line breaks |
| `json` | `.json` | Closed normalized contract with its timing, provenance, binding and cue identities |

These are deliberate interoperability subsets. They do not claim support for
every document accepted by a player, editor or the broader standards. Unsupported
syntax is refused before conversion; permission to discard a known normalized
field does not authorize parsing arbitrary rich syntax.

LRC accepts leading unique metadata keys `ar`, `al`, `ti`, `au`, `by`, `re` and
`ve`. It does not infer language from those keys. Metadata after cues, unknown
keys, offsets, enhanced word timing and multiple timestamps on one line are
refused. Minutes have two to four digits, seconds range from `00` to `59`, and
the fractional part is exactly two centisecond digits, bounded by
`1440:00.00`. Metadata value whitespace is preserved; output metadata keys use
deterministic sorted order.

SRT numeric source labels are retained, including leading zeros. Positioning
suffixes and styling syntax are outside this subset. Input cue order is
preserved, including repeated source labels; the converter does not sort or
repair an invalid timeline. Empty payloads are refused, while whitespace-only
payloads remain text. SRT blank lines delimit cues and cannot be emitted as
embedded payload lines.

LRC and SRT payloads refuse angle-bracket markup, entity-like escapes and ASS
overrides/escapes. Plain text treats such characters literally. A style-rich
subtitle must be handled by a separately supported parser; a loss permission
does not strip unknown formatting heuristically.

WebVTT requires an exact `WEBVTT` header followed by a blank separator. Cue times
use `mm:ss.mmm` or `hh:mm:ss.mmm`. An optional nonreserved cue label is retained;
the only supported markup is an explicit whole-cue `<v Speaker>...</v>` wrapper.
Named references `amp`, `lt`, `gt`, `nbsp`, `lrm` and `rlm` are decoded to text.
Header annotations, `NOTE`, `STYLE`, `REGION`, cue settings, numeric character
references and other markup are refused. Text whitespace and Unicode code
points are preserved without trimming or Unicode normalization.

TTML requires the `http://www.w3.org/ns/ttml` namespace, flat `tt/body/div/p/br`
structure and `xml:space="preserve"` at the root. Descendants may only preserve
that whitespace policy. Paragraphs require explicit `begin` and `end`; ancestors
cannot contribute timing. Accepted clocks are `HH:MM:SS` with optional fractional
seconds, or decimal `h`, `m`, `s` and `ms` offsets that resolve exactly to
milliseconds. Frame/tick expressions and rounding are refused. `br` becomes a
line feed, while text spaces survive XML lexical normalization.

Root `xml:lang` is retained; output explicitly uses an empty value when no
language is supplied. An empty language value means unknown, not an inferred
language. Descendant declarations must agree, and mixed
languages are refused. A paragraph `xml:id` becomes its source label; container
IDs survive as `ttml.tt.id`, `ttml.body.id` and `ttml.div.id` metadata. DTD/entity
declarations, processing instructions, comments, foreign namespaces, styles,
resources, `head`, `span` and broader timing features are outside this subset.
An XML declaration, when present, must declare version `1.0`; any declared
encoding must be UTF-8 and `standalone` must be `yes` or `no`.

## Exact timing and source binding

Every cue has exactly one timing shape:

| Shape | Meaning |
| --- | --- |
| `untimed` | No supplied timestamp |
| `point` | A supplied start time without an end |
| `interval` | Supplied start and end times, with the end strictly after the start |

Times are reduced exact rational seconds on a millisecond grid. The converter
does not use floating-point timestamps, infer a final cue end, stretch a cue to
its successor or create timestamps for untimed lyrics. A start-only LRC document
therefore cannot become an interval subtitle merely by allowing losses.

Cue order, simultaneous starts, gaps and overlaps are checked against the
declared overlap policy. Imports default to rejecting overlap; accepting an
overlapping interval document requires an explicit context. Reordering is not a
repair operation. A cue's internal ID is distinct from an optional source-format
label, such as an SRT number.

An optional context may supply a validated
[`AudioSource`](audio-analysis.md#source-scope-and-exact-time). Binding checks
declared clock/duration relationships without opening media or claiming that
its digest, contents or timing were independently inspected. A successful text
conversion is not proof of synchronization with that audio.

## Provenance and conversion losses

The normalized provenance distinguishes `unreviewed`, `observed_transcript` and
`reviewed_lyrics`. A new text import defaults to `unreviewed`. An explicitly
supplied observed producer or reviewed authority is retained as a declaration;
conversion never promotes a transcript to reviewed lyrics and does not
authenticate the reviewer. Review authority and evidence remain separate from
the converted payload's byte identity.

Conversions refuse known semantic loss unless the caller permits the specific
loss kind. Each permitted loss is reported with its affected field, optional cue
ID and explanation. The allowlist is not a wildcard for unknown syntax or an
instruction to fabricate missing information.

| Loss kind | Permitted projection when needed |
| --- | --- |
| `timing` | Remove supplied timing when exporting untimed plain text |
| `end_times` | Drop interval ends when exporting start-only LRC |
| `precision` | Round millisecond starts to LRC centiseconds, nearest with ties upward |
| `cue_identifiers` | Drop/change source labels or merge cue boundaries that the target cannot represent |
| `speaker` | Remove a speaker field unsupported by the target |
| `metadata` | Remove unsupported document metadata fields |
| `language` | Remove a language declaration unsupported by the target |

Plain output merges multiple cues with two line feeds and records the loss of
their boundaries; it does not pretend the result is still multiple authored
cues. SRT output supplies numeric carrier labels when necessary; replacing an
authored nonnumeric label requires `cue_identifiers`. Precision permission does
not bypass output validation: rounded timestamps must still satisfy the target's
ordering and overlap rules. No permission can manufacture an end time for a point
cue.

Normalized JSON retains provenance, optional audio binding and internal cue
identity. Text carriers cannot represent all of that evidence. The package
retains both normalized documents and enumerates carrier omissions separately;
a standalone `.srt`, `.lrc` or other text payload is not a self-contained review
attestation. Consumers that require provenance should carry the normalized
document and conversion report with the text.

## Bounds and lexical handling

Input must be UTF-8. A leading UTF-8 BOM and CRLF/lone-CR line endings are
normalized explicitly and recorded in lexical facts. The byte digest still
identifies the original input bytes. Unicode text is not transliterated or
silently normalized to a different Unicode normalization form. RTL text, CJK and
emoji remain text; this utility does not claim a rendering or shaping engine.

The profile bounds input to 1 MiB, 10,000 cues, 64 KiB per cue, 32 metadata entries
and a 24-hour time horizon. Plain text's one-cue payload is therefore bounded to
64 KiB. Excessive or malformed input is refused rather than
truncated. TTML parsing prohibits DTDs and external entities/resources; it does
not fetch network content or evaluate arbitrary XML features.
Its XML tree is additionally bounded to 60,000 nodes, depth 6 including the
document, 8 attributes per element and 8 namespace bindings per element.

## Public contracts and local checks

The normalized document is
[`aniflow.timed-text/v1`](contracts/timed-text-v1.schema.json), with separate
[`aniflow.timed-text-context/v1`](contracts/timed-text-context-v1.schema.json),
[`aniflow.timed-text-conversion/v1`](contracts/timed-text-conversion-v1.schema.json)
and [`aniflow.timed-text-registry/v1`](contracts/timed-text-registry-v1.schema.json)
contracts. The [document](contracts/examples/timed-text-v1.example.json),
[context](contracts/examples/timed-text-context-v1.example.json),
[conversion](contracts/examples/timed-text-conversion-v1.example.json) and
[registry](contracts/examples/timed-text-registry-v1.example.json) examples are
synthetic contract fixtures, not evidence of transcription or human review.

`TimedTextDocument`, `TimedTextImportContext` and `TimedTextConversionReport`
expose validated `from_json_slice` boundaries. Directly constructed or ordinarily
deserialized values require `validate` before use. JSON Schema checks structure;
Rust also checks timing, ordering, source bounds and cross-field authority/loss
relationships. Parsing supplied evidence does not independently verify its
digests or claims. The conversion operation separately computes digests of the
input/output bytes that it actually handles.

The library exposes `registry`, `decode`, `encode` and in-memory `convert`.
`decode` accepts bytes, an explicit `TimedTextFormat` and `ImportContext`;
`encode` takes a validated document, target format and `ConversionOptions`.
`convert` returns both normalized documents, the rendered bytes and the report.
Conversion failures retain the discovered losses for callers to present without
silently retrying with broader permission.

`convert_file` provides the file boundary used by the CLI and returns a
`FileConversionOutcome` with the output directory, four artifact paths and report.
Its optional context distinguishes no override from an explicitly supplied
context. The lower-level `decode`/`convert` convenience APIs treat a default
context as no override for JSON; nondefault context must match the embedded
document. None of these paths confer additional review authority.

Normalized document and conversion-report parsers allow up to 8 MiB for evidence
overhead; the original text source and aggregate cue text remain bounded to
1 MiB. Context input is bounded to 1 MiB. A large evidence envelope does not
authorize more source text, cues or timeline duration.

The focused local commands are:

```bash
task timed-text:corpus
task timed-text:schema
task timed-text:smoke ANIFLOW_BIN="target/debug/aniflow"
```

The corpus exercises generated text through Rust integration tests. Schema
checks require an already installed Python `jsonschema` package; the native CLI
smoke uses Python's standard library and a previously built binary. These checks
cover the registered subsets and explicit refusal/loss boundaries. They do not
establish compatibility with every subtitle renderer, broad Unicode rendering
behavior, audio alignment quality or release qualification.

## Format references

The selected WebVTT syntax is informed by the
[20 May 2026 WebVTT Candidate Recommendation Draft](https://www.w3.org/TR/2026/CRD-webvtt1-20260520/).
TTML uses the [TTML2](https://www.w3.org/TR/ttml2/) media-time vocabulary and
[XML 1.0](https://www.w3.org/TR/2008/REC-xml-20081126/) lexical rules within the
documented flat subset. These references describe broader specifications, not
conformance claims for every feature.

SRT and LRC do not have a single universal formal standard. The subset is
explicitly bounded against [Matroska's SRT conventions](https://www.matroska.org/technical/subtitles.html#srt-subtitles)
and the [FFmpeg 6.1 LRC reader](https://ffmpeg.org/doxygen/6.1/lrcdec_8c.html).
aniflow does not execute FFmpeg for text conversion or adopt every permissive
behavior of another parser.
