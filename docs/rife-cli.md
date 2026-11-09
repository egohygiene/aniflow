# Standalone serial RIFE commands

`video rife` delivers the existing [fixed runtime facade](rife-runtime.md).
Supply original indexed RGB8 PNGs and closed configuration with exact local
tool/model digests, supported host/device declarations and finite bounds.
It does not accept whole-video input or invent an MP4 adapter.

```bash
aniflow --output json video rife plan \
  --frames-directory "/absolute/original frames" --frame-index index.json \
  --configuration rife.json
aniflow --output json video rife run \
  --frames-directory "/absolute/original frames" --frame-index index.json \
  --configuration rife.json --output-directory "/absolute/separate runs"
aniflow --output json video rife resume \
  --frames-directory "/absolute/original frames" --frame-index index.json \
  --configuration rife.json --run-directory "/absolute/existing RIFE run"
aniflow --output json video rife status \
  --run-directory "/absolute/existing RIFE run"
```

Path flags resolve against the current directory; embedded tool/model paths
retain canonical bindings. Configuration is bounded closed JSON, at most 64 KiB.
Plan hashes/inspects actual PNG/index/component bytes and resource admission
without a process or workspace. The older pure `video interpolation-plan`
remains a configuration/index-only preview without full frame/component checks.

Run/resume reuse ordinary v3 progress, cancellation, checkpoints and validation,
with finite SDK defaults unchanged. Changed inputs/pins/settings refuse stale
resume. New-run recovery begins at Started; resume keeps its named root even on
early settings refusal. Status checks current fixed-profile native state without
creating authority, repairing partial initialization or invoking tools. No model
download/install/fallback or partial-pair adoption occurs. Disabled RIFE cannot
become an executing runtime; ordinary recipe/pure planning retains explicit bypass.

Output is a separate frame/index set with the existing same-duration 2x clock,
cut/endpoint holds and original inspection. For video assembly use the existing
[accepted carrier](recipe-accepted-rife.md) or [fresh recipe child](recipe-rife-child.md).
Initial rate/color/count and serial worker bounds remain unchanged. Declared
RAM/device facts are not measured host truth or enforced GPU memory quotas.
Combined spatial/interpolation, short-preview/finer recovery and broader native/
platform/visual qualification remain #110 work.

Envelopes use `video_rife_plan`, `video_rife_run`, `video_rife_resume` and
`video_rife_status`. Full plans/outcomes/status/recovery contain private locators.
Actual run/resume would invoke the selected provider; none ran for this checkpoint.
Four parser/refusal/recovery cases are authored, uncompiled/unrun. Tests/builds/
validators/CLI/SDK/native/provider/media/model/CI stay deferred to
[flow #88](https://github.com/egohygiene/flow/issues/88). Alignment cancellation/
start-marker remains unresolved, cause unproven. No native, human-review,
resource/visual/platform qualification or release is inferred.
