# Offline vocal separation — implementation checkpoint

Owner: [aniflow #8](https://github.com/egohygiene/aniflow/issues/8).
Base: `8a88b0e96c2ac89da9b26b579300e84aa80aa762`.

## Contract

Implement one typed `aniflow/audio.separate` provider through the existing
Pipeline v3 direct-argument ABI. The public Rust library owns planning,
bounded execution, process-group cancellation, artifact validation, checkpoints,
status, and compatible resume. A canonical task delegates to that CLI/library
path; no second scheduler, mutation path, or sibling source dependency is added.

The first profile accepts PCM16 WAV (one or two channels, 8–192 kHz, at most
600 seconds / 256 MiB) and uses Demucs 4.0.1, `htdemucs_6s`, CPU, two-stem vocals,
zero random shifts, and a prepared local model repository. Outputs are explicit
vocals and accompaniment WAV ports plus versioned JSON evidence. Decode every
sample, reject incomplete RIFF/PCM data, compare durations using a 20 ms tolerance,
and observe checksums, sample counts, sample rate, channels, and bytes.

Preparation downloads nothing. Record the local Python/FFmpeg executable,
model bag, and checkpoint digests. Run with the Torch/HF offline flags and an
explicit local `--repo`; missing or changed assets refuse before separation.
The canonical run/resume task rechecks external material because a static
registration document cannot observe changes in its referenced model cache.
Keep that cache outside the generated workspace. Preserve venv interpreter
path semantics, immutable sources, and no-clobber output publication.

Offline declarations and environment flags are not an OS network sandbox.
Dependency authenticity and a hostile same-user environment are outside this
contract. Actual model inference quality, native macOS, MSRV and release
qualification require separately observed evidence.

## Review checkpoints

1. Draft: fixed scope, provider manifest/pipeline, preparation/delivery scaffold.
2. Implementation: strict provider/configuration/evidence, synthetic ABI tests,
   canonical task and operator documentation.
3. Validation: focused failure/resume cases, full Rust targets, formatting,
   Clippy, contract/name checks and existing synthetic FFmpeg smoke. Record
   unavailable gates and publish the exact review tree. Maintainer owns merge.

At the first draft checkpoint, the provider and tests are still in progress;
the scaffold is not runnable or qualified. This status will be replaced by
executed local evidence before ready-for-review handoff.

## Acceptance evidence

Tests must exercise source preservation, spaced/Unicode paths, missing and
changed cache refusal, exact direct argv, offline flags, valid stems, corrupt /
partial / wrong-duration stems, nonzero exit, bounded cancellation, no-clobber,
checkpoint/status, and compatible resume without relaunch. Fixtures are
synthetic and must not claim actual model inference compatibility.

## Decision impact

This applies ADR-0003's existing typed external-tool boundary and ADR-0004's
versioned contracts. No new execution owner or accepted architectural decision
is introduced; the narrow profile and its limits are proposed for review here.
