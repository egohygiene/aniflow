# Accepted RIFE output to master and delivery — source checkpoint

This #110 checkpoint adds a public, explicit accepted-output handoff to the
existing assembly owner. It is source only: no build, test, schema validator,
RIFE, model, media, CLI, SDK or CI execution was performed. Four regression
cases are authored and remain uncompiled/unrun. #64 qualification and its
known alignment cancellation/start-marker failure remain open; cause unproven.

`video_assembly::rife::bind_accepted_run` takes a currently complete accepted
RIFE v3 run, its exact configuration and original initial frame-index file.
It rechecks retained plan/manifest, producer and configuration locks, the
initial index input, current accepted output index and frame directory. It
authors a new private no-clobber `aniflow.rife-assembly-index/v1` outside that
run. The [Rust example](../../examples/bind_rife_assembly.rs) prints the exact
directory and carrier to supply to existing `video assemble-frames`.
The accepted RIFE run must remain available and unchanged. Its existing writer
lock is held across assembly facade execution; no tool/model is rerun to bind.

The carrier preserves original temporal inspection and RIFE source/midpoint/
cut/hold mapping. A separate `aniflow.rife-derived-clock/v1` declares exact 2x
cardinality and cadence over the same complete zero-origin duration. Ordinary
unchanged-clock temporal validation keeps its original semantics. The new
`aniflow.validation/derived-temporal-media/v1` contract independently observes
source/output bytes, checks every derived frame interval and exact original
audio extent. No fabricated original inspection is passed to validation.

Assembly retains its bounded source/tool/PNG checks, full master RGB decode
comparison, delivery decode and geometry checks, MP4 atom checks, exact AAC
packet payload/timing/extradata/priming comparison and normal immutable v3
acceptance gates. No `-shortest`, resampling, audio stretching, padding or audio
re-encoding is added. Master and delivery remain distinct accepted artifacts.
Original clocks/mix, prior runs and creative approvals remain unchanged.

The initial subset is enabled existing serial RIFE 2x output: one RIFE stage,
an explicitly initial original index, 1–50,000 source frames / 2–100,000 output
frames, complete contiguous zero-origin CFR, unchanged SDR geometry and the
existing MP4/MOV source plus no-audio/AAC-LC assembly subset. A loose RIFE output
index, derived-to-upscale composition and ambiguous/missing/stale accepted
evidence are refused. The recipe does not automatically insert interpolation.
Broader rate/profile/platform support, finer RIFE recovery, recipe integration
and native/resource/visual qualification remain on #110.

Optional derived fields are omitted for existing original/spatial contracts,
preserving their canonical serialization. Updated schema digests are new lock
material: this does not promise reuse across changed tool/schema/config pins.
Source publication does not adopt proposed ADR-0011 or approve image quality.
