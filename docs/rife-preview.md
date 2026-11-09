# Bounded RIFE preview sequence

Source checkpoint for #110, uncompiled and unrun. This adds a short actual
native PNG-sequence preview through the existing RIFE pair worker and ordinary
v3 runtime. It does not require executing a full interpolation first. It is not
a preview MP4, a full output index, a final-video handoff or perceptual approval.
All native/device/performance/quality qualification remains in flow #88, last.

## Explicit selection

Use the same complete original frame directory/index and pinned enabled RIFE
configuration as `video rife plan/run`. Select a nonempty half-open original
source-frame window of at most 120 frames and at most ten exact source seconds.
No inferred highlight selection, model installation, provider fallback or new
cross-repository orchestration is added.

Future operator examples, not executed here:

```sh
aniflow video rife preview-plan --frames-directory /work/original-frames \
  --frame-index /work/original-rife-index.json --configuration /work/rife.json \
  --first-source-ordinal 30 --end-source-ordinal 90
aniflow video rife preview-run --frames-directory /work/original-frames \
  --frame-index /work/original-rife-index.json --configuration /work/rife.json \
  --first-source-ordinal 30 --end-source-ordinal 90 --output-directory /work/previews
aniflow video rife preview-status --run-directory /work/previews/EXACT-RUN
```

The run parent must already be a canonical directory separate from input,
provider, tool and model paths. `preview-resume` takes unchanged indexed inputs,
configuration/window and `--run-directory`; it uses the same v3 locks, checkpoint,
progress/recovery and process-group cancellation as full RIFE. A failed stage
restarts according to ordinary stage acceptance, without a per-pair journal or
adoption of loose PNGs. Use generic `status-v3` for unfinished work. SDK `status`
returns native state; SDK `observe` and CLI `preview-status` require current
complete native acceptance and recheck original and output bytes.

## Original clock and limited work

The complete original index and temporal inspection remain unchanged, and all
indexed original PNGs are admitted/hashed. Full-input I/O and snapshot costs
therefore remain. Native model work and output PNGs are limited to twice the
selected source window, at most 240 derived frames. The existing midpoint,
cut-hold and endpoint-hold renderer is shared with the full provider.

Filenames/ordinals retain their global derived positions, not a new zero origin.
Every frame records an exact original presentation interval, source left/right
ordinals and original/interpolated/cut/endpoint origin. The report stores the
complete source plan separately from the selected window. The final midpoint
may read one right-context source frame beyond the selected window; this keeps
the same mapping as the full run. A true source endpoint uses the existing hold.
Original shot boundaries are honored; none are inferred or rewritten.

The new report and frame media types are deliberately distinct from full-run
`rife-output-index/v1` and assembly carriers. A preview does not become complete
interpolation or relax an existing full assembly/review gate. Audio is not
processed or muxed by this frame stage. Full original audio synchronization and
picture assembly remain their existing owners.

## Pins, resources and authority

Tool, local model, explicit CPU/Vulkan selection and host-memory declarations
reuse the existing RIFE settings/manifest/inventory owner. Private pinned tool,
model and source-pair snapshots, bounded subprocess capture/deadline, exact PNG
inventory/geometry/digests and current native acceptance apply. Preview overall
deadline is at most one hour; artifact admission remains finite with at most
256 generated/private files. Memory estimates remain caller-attributed and
uncalibrated, not an OS-enforced RSS/VRAM quota or a crash-prevention guarantee.
Native private work and ordinary v3 failure evidence are retained on failure.

Current accepted observation checks complete original index/directory, selected
window/configuration/lock, checkpoint/delivery hashes, report shape, original
clock/held-frame mapping and PNG directory bytes. It never runs native tools or
repairs state. Source/index/window/tool/model/provider/settings changes refuse
resume. Rendering quality, temporal coherence, shot behavior, device portability,
performance, disk pressure, forced cancellation and corruption qualification are
unrun. `native_qualified` stays false and creative approval is `not_performed`.
No source replacement, automatic full-run approval, publication or CI execution.
