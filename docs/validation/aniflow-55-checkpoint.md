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
6. Push final validation and synchronize #55, parent #13 and Flow #11. Mark the
   PR ready for maintainer review and stop; do not merge it or start #45.

Native macOS, other FFmpeg builds and release/EBU qualification remain separate
gates. Do not wait for or poll hosted CI.
