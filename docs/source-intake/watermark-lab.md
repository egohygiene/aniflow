# Optional watermark-lab reference (#231)

Disposition: retain the supplied lab as a separate optional experiment. There
is no watermark-lab module or command in the inspected main source; the
[current public module list](../../src/lib.rs) and existing provider/runtime
owners are not extended by this intake. The
[issue](https://github.com/egohygiene/aniflow/issues/231) explicitly allows this
reference-only disposition. Ordinary delivery remains independent of the lab.

## Preserve both prototype identities

The [archived source](README.md) is wrapper2.0.0, 28038 bytes, SHA-256
`c87543367c12af94b0f4436d12cf6cbee37430c8de254044e909a4afd7e659e2`.
Historical [#39](https://github.com/egohygiene/aniflow/issues/39) identifies a
different standalone prototype by SHA-256
`62134b6feea723983a72442b3f51fcc57e1bf3c562c19db8c85804fcdc2de3bf`.
Do not overwrite that receipt, call the two artifacts identical or transfer
the older reported synthetic results to this copy. The old artifact's bytes
were not available for a direct diff in this intake.

Both references name backend commit
`d25cada76bc39bab59cd7a41c29028b1cc98a5f2`. That identifies a dependency source,
not a tested host, model or detector. The archive records its observed resolved
VAE model revision/hashes after setup rather than pinning those model bytes in
advance; its model name alone is not an immutable model identity.

## Useful experimental behavior and disposition

| Observed archived behavior | What to preserve and where it belongs |
| --- | --- |
| Separate `prepare`, `setup`, `process` and private worker paths; default60 frames, maximum240, explicit one-based start frame. A new workspace retains a silent losslessly encoded reference and source/reference hashes. | Keep bounded reference/control selection and no overwrite. The historical issue's request for a `doctor` is not implemented as a separate command in this copy. Future source/clip admission should reuse existing temporal/frame owners; source and approved master stay immutable. |
| Preparation refuses non-yuv420p, recognized HDR, rotation and geometry not divisible by8, and validates requested frame count/rate/PTS/geometry. | Preserve explicit unsupported-profile refusals. “Full resolution” is relative to that admitted source; do not silently apply upstream512-pixel/12-fps defaults. Structural checks are separate from quality and native qualification. |
| Setup explicitly creates an isolated virtual environment, installs the pinned backend with video/diffusion extras, downloads the VAE, records resolved revision/digests and observed dependencies. Processing requests cached/offline operation and verifies those observations. | Retain a separately authorized preparation boundary. Do not turn optional model acquisition into a dev dependency or ordinary delivery step. No backend install, model/cache access or upstream fetch was performed during this review. |
| The worker selects MPS/CUDA/CPU explicitly, uses batch1/seed0 and recorded noise, enables128/256/512-pixel VAE tiling with25% overlap, and overrides selected backend internals/encoder threading. | Retain this exact experimental configuration as evidence. Private API hooks, ffprobe-only profile substitution and encoder changes differ from upstream calibration and can break across versions; do not expose them as a supported general provider. |
| Workspace `flock` is inherited by the worker. A separate process group has sampled RSS, wall-time and free-disk checks; MPS also has allocator limits and thread/cache controls. Unique attempts retain logs/resource observations. | Preserve explicit finite resource and failure evidence. Sampling may miss spikes; RSS and MPS budgets are not additive or an OS/GPU-wide quota. Current runtime [process ownership](../../src/audio_inspection/process.rs) and [v3 lifecycle](../../src/provider_runtime.rs) remain the owners for any future integration, not a second Python runner. |
| Every attempt verifies reference/model identity, writes separate candidate/metrics/resources/report and an aligned preview. Output dimensions/frame count/rate/PTS are checked. | Keep independent output observations and exact hashes/configuration. The preview is a reduced display aid, not a full-resolution quality verdict. Failed candidates must not become accepted delivery just because an output file exists. |
| The final report says structural output validated, human quality pending, pixel/audio watermark verification not tested and release not approved. | Preserve these independent facts. No compatible detector is supplied, no full-song run occurs and the experiment is silent. Metadata cleanup is unrelated to invisible signal evidence. |

The older #39 receipt described resource enforcement as missing. This archive
does contain tiling and watchdog code, so preserve that later source fact
without rewriting the earlier dated observation. Merely reading that code does
not establish that the limits work on Apple MPS, CUDA, CPU or the intended4K
input, or that a particular crash was caused by memory exhaustion.

## Concrete go/no-go boundary under #212

The current disposition is **reference-only; integration not selected**. No
measured native quality/detector evidence has been supplied for this artifact.
The optional experimental follow-up retained from #39 remains under
[#212](https://github.com/egohygiene/aniflow/issues/212):

1. Freeze one exact backend/model/configuration/device identity and one bounded
   source excerpt. Validate setup/doctor refusal, cache/hash drift, locking,
   timeout/resource/cancellation and interrupted-output behavior using synthetic
   fixtures before an explicitly selected real-host experiment.
2. Record native dependency/device success or failure, actual resolution/rate,
   source/reference/candidate hashes and resource observations. Do not infer
   device support or throughput from an exposed backend option.
3. Independently review full-resolution text, line art, faces, color, edges,
   transitions and flicker against the reference. Keep failed and inconclusive
   results alongside positive ones; a structural pass cannot approve quality.
4. If applicable detector evidence is available, bind its algorithm/version,
   exact input/output hashes and scope to a dated result. Otherwise effectiveness
   stays unavailable/not-tested; no universal removal or undetectability claim
   follows. An external oracle/upload is a separately authorized operation.
5. Consider one optional versioned provider through existing v3 contracts only
   after that evidence supports its exact profile. A silent visual experiment
   cannot claim original-audio verification or replace a full master. Creator
   watermark embedding remains separate retained #81 scope under #212.

The original source stays available by immutable link. Only source/hash/link
inspection was performed for this intake; no archived worker, model, device,
detector, media, setup or remote oracle was run, and no output is approved.
