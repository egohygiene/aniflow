# High-rate true-peak checkpoint

Issue [#55](https://github.com/egohygiene/aniflow/issues/55) is the bounded
high-rate follow-up to #44, selected explicitly by the maintainer before #45.

## Draft checkpoint 1

The maintainer explicitly authorized merging PR #54. It was squash-merged at
`9e596f3fbc1d94de5a7cfc05692db2941a554a8f`; fresh main has tree
`d88c574a9ec005d3942e97392462e5b6a08d127d`. Repository instructions and live issue
scope were refreshed before this branch. Authorization to merge #54 does not
authorize merging the new #55 PR.

The target is honest true-peak evidence at 88.2, 96, 176.4 and 192 kHz while
preserving source bytes, the existing lower-rate profile, bounded direct tool
execution and the separate zero-extension pass. The normalized audio envelope
and technical inspection remain unchanged. Explicit interpolation candidates
must first pass generated intersample-tone, terminal-impulse, silence,
very-short-input and asymmetric-channel probes with stated tolerances.

Method/command evidence and provider identities will distinguish the new
algorithm from #44's 192 kHz-target behavior. Historical v1 signal reports must
remain readable without silently adopting the new semantics. New execution
must not reuse an older implementation's checkpoint as new qualification.
No real-media mutation, model download or compliance claim is authorized.

## Recovery and finish plan

1. Read this checkpoint and current draft PR, then inspect the current branch.
2. Finish independent numerical probes and pin a bounded explicit method.
3. Update provider/library contracts, schemas/examples, CLI documentation and
   exact resume identities coherently. Preserve unsupported-rate outcomes
   outside the newly qualified subset.
4. Add synthetic numerical and malformed-tool/method refusal coverage; retain
   existing cancellation and subprocess/capture bounds.
5. Push an implementation checkpoint, run meaningful local repository checks
   and real-tool smoke, then record exact evidence and unverified gates.
6. Push final validation and synchronize #55, parent #13 and flow #11. Mark the
   PR ready for maintainer review and stop; do not merge it or start #45.

Native macOS, other FFmpeg builds and release/EBU qualification remain separate
gates. Do not wait for or poll hosted CI.

## Draft checkpoint 2

The implementation is present in [PR #56](https://github.com/egohygiene/aniflow/pull/56):
explicit four-times double-precision SWR interpolation, pinned astats export,
exact padded frame-count and non-finite-sample checks, and a source-bound v2
signal companion. The original v1 schema/example and parser semantics remain
unchanged. Provider/runtime capability/implementation identities advance to v2;
the normalized evidence-family contract remains v1.

Fourteen focused signal units, nine public integration tests, eleven independent
schema tests and 48 published JSON documents pass. The integration fixtures
exercise all four rates, malformed sample-count refusal, version-separated
parsing, compatible resume and a validated synthetic legacy-plan refusal.
Independent review found no blocker. Initial direct-tool numerical probes pass
28 generated fixtures; a 600-second 192 kHz mono probe preserved source bytes
and reported its exact interpolated frame count.

The complete repository smoke is now exercising 28 high-rate cases through the
public CLI, alongside the existing signal/inspection/video paths. Final all-target,
strict Clippy, MSRV, docs and package checks and the final receipt remain pending.
Recover from this pushed implementation, finish those checks and handoffs, then
stop for maintainer review. This draft is not release qualification.
