# Native reviewed-lyrics alignment provider

This bundle declares the first-party, bounded PocketSphinx alignment stage. Use
`aniflow::audio_alignment` or `aniflow audio align-lyrics` to compose technical
inspection, optional verified stem lineage, and candidate alignment through
Pipeline v3. The provider executable uses the fixed
`--aniflow-invocation <absolute-request-path>` ABI.

The initial profile is PocketSphinx 5.1.1 with caller-supplied, pinned `en-us`
acoustic resources and a pinned `cmudict-en-us` pronunciation dictionary. Audio
is mono PCM16 WAV at 16000 Hz. The reviewed lyric document is an immutable input;
its exact cue text, review authority, and source identities remain explicit.
The admitted lyric tokens are ASCII English words, with internal apostrophes;
other lexemes require an explicitly reviewed text revision. Word and cue timing
proposals are probabilistic candidates. They do not establish
that words were sung or that timing was reviewed, and calibrated confidence is
unavailable. Unmatched and ambiguous words remain explicit.

The fixed outputs are `alignment.json` and normalized `analysis.json`. The
alignment companion retains the reviewed document and separate candidate
observations. Timed-text exports retain the original words and require explicit
loss authorization where a carrier cannot preserve the complete document.

## Explicit local dependencies

Configuration pins the executable and dictionary by SHA-256 and each acoustic
resource by name, byte count, and SHA-256. These pins bind caller-supplied local
bytes; they do not independently authenticate upstream origins or review claims.
The model component versions `1.0.0` name this integration profile. Model revision
and exact component identities remain explicit in configuration and evidence.
No executable, dictionary, or model is downloaded. Missing, changed, unsupported,
or over-limit dependencies produce typed preflight diagnostics. Planning and
resume reobserve dependencies before resolving or reusing run-local checkpoints.
Read-only Pipeline v3 status does not launch the aligner or prove current
readiness.

Execution copies explicitly pinned resources into private staging, invokes a
direct argument vector, disables inherited language-model fallback with an
explicit unused private path, bounds captured output and the child deadline, and
rechecks immutable inputs before publishing evidence. The native executable is
trusted code. Manifest effect declarations and private staging do not establish
an operating-system sandbox or complete native dependency closure.

The acoustic resource set is closed to seven sorted file names and at most
64 MiB total; the dictionary is at most 16 MiB. Reviewed JSON is at most 64 KiB,
with at most 512 tokens and a 16 KiB normalized phrase. Tool capture is bounded
to 1 KiB–1 MiB and the child deadline to 120 seconds. Source inspection retains
its 256 MiB and 600-second limits.

The published JSON examples use synthetic marker dependencies and synthetic
observations. They demonstrate contract shapes and identity derivation. They are
not an installable distribution, recognition accuracy evidence, native platform
qualification, or proof that a human performed the embedded review attestation.
