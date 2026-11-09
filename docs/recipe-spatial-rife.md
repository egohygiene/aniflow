# Spatial upscale, RIFE and assembly

Source checkpoint for #35/#110. Select the existing Upscayl or target-resolution
policy and enabled `interpolation_run` in the fixed music-video configuration.
The order is preparation/repair → selected spatial processing → RIFE → optional
prepared-frame review → assembly. Both indexed and source entries use that tail.
There is no ordering engine, nested interpolation, or RIFE-before-upscale path.

Fresh interpolation binds the complete current native Upscayl run, exact accepted
index and every PNG. A loose spatial index is insufficient. The spatial adapter
reuses the serial pair renderer, pinned components and ordinary v3 runtime,
checkpoints, cancellation, progress and resume. Its distinct output-index type
prevents promotion through original-frame validation. Input/model/pair/output
storage and bounded evidence are admitted before native work.

The assembly carrier keeps the original frame index and temporal inspection
unchanged. Optional `spatial_input` records the accepted spatial lineage and
actual dimensions; a separate exact full-duration 2x clock describes RIFE output.
Held frames must match upscaled pixels. Existing carrier omission remains the
original-frame contract. Current Upscayl and RIFE acceptance are rechecked at the
handoff; assembly owns the RIFE writer while consuming it, avoiding a recursive
recipe-parent acquisition. The selected original audio remains assembly-owned.

Prior accepted Upscayl reuse is supported under its existing disabled-repair and
no-further-upscale rules, followed by fresh RIFE. External original-frame RIFE
reuse cannot approve different spatial pixels. Pending child plans still wait
for actual accepted outputs; initial planning does not fabricate future hashes.

Resource limits are unchanged and uncalibrated. Projected geometry is admitted
before expensive spatial work when known. Conservative/auto configurations can
refuse 4K, especially on smaller-memory Macs. This checkpoint does not provide a
RIFE-before-upscale alternative or claim all selected combinations are feasible.

Six synthetic regression cases are authored, uncompiled and unrun. No tests,
builds, format/schema checks, provider/media/model/CLI execution or CI ran.
Native memory/GPU/visual suitability and actual Akashic runs remain deferred to
#64 and suite flow #88. No creative approval or publication is inferred. The
known alignment cancellation/start-marker failure remains unresolved with cause
unproven. ADR-0011 and ADR-0012 remain proposed.
