# Optional original-audio observation in the fixed recipe

Source checkpoint for #35. Add `"audio_signal":{"maximum_pcm_bytes":67108864}`
to the indexed recipe (or the source entry's nested `recipe`). Omission preserves
the old canonical intent bytes. This fixed prelude uses the existing
`delivery_audio::source_signal::measure` owner, pinned assembly tools and native
provider, after accepted source extraction when that entry is used. It initially
requires the existing 48 kHz mono/stereo AAC source profile. Unsupported or missing
audio refuses explicitly. There is no ASR, normalization, resampling, downmix,
trim/pad, remix or source/master replacement.

The private `audio-signal` observation contains the exact original source,
complete float32 PCM, accepted #13 child and separately labelled native meter
receipt. An exclusive hash-bound marker joins the recipe intent, exact request
and receipt only after measurement completes. Read-only status rechecks retained
source/PCM bytes and the child's current native acceptance; resume reuses only
that exact completed observation. Partial initialization is preserved and blocks
automatic replay. Source entry pending extraction has no fabricated audio result.

Both recipe status/outcome envelopes expose optional `audio_signal` locators and
digests. A selected observation is required for recipe completion. Native-v3's
existing unavailable meter fields are unchanged; the separate receipt records
historical meter observations, never fresh meter execution on status, algorithm
qualification, human approval or delivery acceptance. Existing bounded workspace
admission belongs to the specialized operation, not a second recipe runtime.

Two recipe admission regressions are authored, uncompiled/unrun, alongside the
existing source-signal cases. All execution and qualification remains deferred
to #64. Its known alignment cancellation/start-marker failure remains unresolved;
the cause is unproven. No dependencies, CI, tests, builds, validators or native
tools ran for this checkpoint.
