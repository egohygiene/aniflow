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
