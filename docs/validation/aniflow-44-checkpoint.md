# Signal measurement checkpoint

Issue [#44](https://github.com/egohygiene/aniflow/issues/44) is checkpoint 3 of
parent [#13](https://github.com/egohygiene/aniflow/issues/13).

## Draft checkpoint 1

Fresh main `71d0df8df669a0b7f5b47b6998d1c7819ecda442` contains the maintainer's
merge of #53, completing #43. Live dependencies, the suite handoff and repository
instructions were refreshed before branching. The maintainer owns all merges.

The selected design adds a bounded signal stage after the existing technical
inspection stage in Pipeline v3. It reuses exact source/tool authority, private
snapshots, process limits, candidate acceptance and run-local checkpoints.
A focused public library facade and canonical signal analysis CLI/task selection
produce a strict signal companion and normalized evidence. No general runtime,
new normalized envelope version, preview or source replacement is introduced.

Native sample parsing measures per-channel sample peak, RMS, crest factor and
exact silence/threshold-clipping regions. Pinned FFmpeg measures integrated and
short-term loudness, loudness range and true peak. Units, scope, window lengths,
thresholds and unavailable states remain explicit. Source bounds remain PCM16
mono/stereo RIFF WAV, at most 600 seconds and 256 MiB; signal rates must divide
into exact 100 ms hops. All fixtures are synthetic.

Initial real-tool probes exposed two boundaries: silent/short-input loudness
sentinels cannot become successful zero values, and FFmpeg 6.1's ebur128 true-peak
path does not flush end-of-source interpolation by itself. A separate peak-only
pass with fixed 100 ms zero extension will be validated on trailing impulses;
its padding must never contaminate the unpadded loudness pass or source bytes.
Numerical tolerances and supported semantics will be recorded after validation.

## Recovery plan

1. Read this checkpoint and the current PR description; fetch the branch and
   inspect existing implementation instead of restarting.
2. Finish the closed measurements/settings contract, native sample analysis,
   bounded loudness/peak parsing, source-bound provider and two-stage facade.
3. Keep schemas, CLI/help/tasks, synthetic fixture IDs and support matrix aligned.
4. Verify silent/too-short sources, tone/impulse/channel asymmetry/clipping,
   precise regions, missing/changed tools, malformed/non-finite evidence,
   timeout/cancel/capture bounds and exact compatible resume.
5. Push implementation and validation checkpoints. Run meaningful local checks
   and real FFmpeg synthetic smoke; do not poll hosted CI.
6. Mark the finished PR ready and stop for maintainer review/merge. Keep #13 open.
   Native platforms, broader audits and release qualification stay explicit gates.

## Draft checkpoint 2

The native PCM and FFmpeg adapters, strict report/settings schemas, public
library, CLI/tasks, two-stage pipeline, examples and fixture generators are
implemented. Library/all-target compilation, strict Clippy and eight native
sample/parser unit tests pass. Focused integration checks have passed plan,
execution, compatible resume, changed-authority refusal and cancellation
recovery; the remaining failure matrix and real-tool numerical smoke are in
progress. This checkpoint is not a final qualification receipt.

The fixed support policy now makes true peak unavailable above 48 kHz, adds
100 ms zero extension only to its separate peak pass, requires 60 seconds and
at least ten qualifying short-term windows for loudness range, and declares
the short-term export floor separately from integrated-loudness gating.
A successful pipeline may produce a partial normalized analysis when a
requested measurement is explicitly unavailable.

Next: finish all focused and real-tool fixtures, validate emitted schemas,
run the repository local checks, record the final receipt and update handoffs.
Draft PR #54 remains the durable recovery point. Do not merge it.

## Final local validation checkpoint

Implementation/test commit `f188c14f611421ce94a8e3d177fbd1db1578ac3d`
and remote commit `85fe2f2c797876b63f5cb21e4f85f707794392ec` share tree
`0ff1b2ee0c7924aad1191ec8a1086dd6a332229c`. This final documentation checkpoint
adds the [local validation receipt](aniflow-44-local.json) and synchronized
support/roadmap handoff; the PR is ready for maintainer review after it is pushed.

All 193 Rust 1.94 tests pass. Eight signal units and six signal integration tests
also passed on Rust 1.85.1. The full generated-media repository smoke passes,
including all ten signal fixtures with real FFmpeg 6.1.1. Strict Clippy,
formatting, naming, independent schemas, 46 contract documents, warning-free
Rust docs and source-package compilation passed. The receipt distinguishes the
initial test-expectation/startup-deadline failures from their successful fixes.
Independent final review found no blocking correctness issue.

Native macOS, other FFmpeg builds, full MSRV suite, hosted CI, EBU compliance
and release qualification remain unverified. Above-48-kHz true-peak qualification
is separately tracked in [#55](https://github.com/egohygiene/aniflow/issues/55).
The adapter reports explicit unavailability for that rate range today.

Stop for maintainer review/merge of [PR #54](https://github.com/egohygiene/aniflow/pull/54).
After fresh live-state verification, #45 is the recommended next checkpoint;
#47 remains an independent lane. Keep parent #13 open and preserve its later
product/release order. No real media, model download, hosted CI polling, merge
or release occurred in this checkpoint.
