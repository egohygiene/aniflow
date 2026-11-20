# Measure Upscayl before choosing a full-video policy

`video upscale-recovery resources` reports actual source, index, PNG and pinned
component sizes alongside the same conservative calculation used by fresh
per-frame and bounded-chunk admission. It does not launch tools or create an
output directory. It also explains a policy that is too small, so a denied
plan does not require guessing smaller PNG limits.

```sh
aniflow --output json video upscale-recovery resources \
  --source-video /absolute/source.mp4 \
  --frames-directory /absolute/frames \
  --frame-index /absolute/index.json \
  --configuration /absolute/upscayl.json \
  --storage-parent /absolute/existing-output-volume
```

The input directory must match its complete existing index. The report hashes
all PNGs and checks their structure/CRC; its `complete_decode_performed: false`
explicitly separates that measurement from the provider's full FFmpeg decode.
`components` measures executable, decoder, both model files and selected runtime
libraries. The exact managed receipt/catalog identity and runtime file pins
remain in provider configuration and checkpoint compatibility. Installed managed
namespaces are immutable output exclusions, including during initial recipe
extraction and direct provider execution.

The machine command is `video_upscale_recovery_resources`; its payload follows
[the resource report contract](contracts/upscayl-resource-report-v1.schema.json).
The public library exposes `upscayl_recovery::resources::observe` and an exact
`measure_output_pngs` directory sampler. The latter still grants no acceptance:
only the ordinary provider/runtime can authorize complete decoded output reuse.

| Term | Interpretation |
| --- | --- |
| Original source/index/input PNGs | Measured lengths already present; included in the working footprint |
| Original component files | Measured external dependency/cache lengths; reported separately from new free-space requirements |
| Output candidates and accepted copies | Three complete output PNG bounds per source frame, preserving the existing conservative policy |
| Append-only metadata | Per-frame `(N+1)² × 16 KiB`; chunks `(units+1) × (frames+units+1) × 16 KiB` |
| Component copies | Two complete measured component sets: bounded active scratch plus one retained failed attempt |
| Active PNG scratch | Selected checkpoint size times the input/output per-file caps |
| Shared report allowance | Four maximum 64 MiB report documents |
| Required free space | Additional fresh-run bound plus the selected minimum free margin, compared with a current filesystem observation |

These are conservative admission terms. Free space is shared and can change
immediately; the adapter does not reserve a disk or enforce a filesystem quota.
The report includes actual index bytes in the working footprint. A 2-frame
chunk size admits at most 2,048 frames because of the independent 1,024-chunk
cap; a 256-frame chunk still admits at most 100,000 total frames. Per-frame mode
retains its 1,024-frame cap. PNG/index/plan and artifact limits can tighten them.

The `geometry` section always includes the model-native 4× dimensions, even
when requesting 2× or 3× output. RGB24 and float32 RGB representation sizes are
**single-image lower bounds**, not measured NCNN working sets or GPU-memory
budgets. They exclude tensors, tiles, weights, driver allocations and other
host activity. `measured_device_or_rss_bytes` stays unavailable until a separate
native measurement is recorded. The adapter writes no separate native4× PNG;
it must not invent that as measured persistent disk usage.

After a failure, include existing occupancy separately:

```sh
aniflow --output json video upscale-recovery resources \
  --source-video /absolute/source.mp4 \
  --frames-directory /absolute/frames \
  --frame-index /absolute/index.json \
  --configuration /absolute/upscayl.json \
  --storage-parent /absolute/existing-output-volume \
  --retained-run-directory /absolute/retained-run
```

The optional retained inventory measures logical file lengths twice, including
private `.upscayl-frame-*` / `.upscayl-chunk-*` scratch. It does not measure
allocated blocks, follow symlinks, hash/decode those outputs, acquire a writer
lock or establish that a named directory is an accepted run. This observer
bounds its walk to one million entries, 64 MiB of path text, depth below 64,
and 16 TiB of logical file data. A changing inventory refuses the observation.
Repeated explicit failures can retain further attempts; there is **no finite
lifetime retry allocation**. Resume keeps its separate immediate free-space
checks and never removes failures or lowers settings automatically.

Ordinary v3 workers place private frame/chunk attempts under the canonical
`provider-work` directory beside the same attempt's disposable `candidate`
directory. This keeps failure receipts and partial files outside generic
candidate cleanup, without granting them checkpoint acceptance. The provider
uses that location only after checking the exact sibling relationship;
direct library calls retain their assigned-output-parent behavior. Successful
private attempts are removed as before, and both volumes receive storage checks.

Measure the intended model's real output PNG distribution and device/RSS peak
on a bounded representative excerpt before selecting caps for a longer video.
An output over the selected PNG cap fails, even when earlier sample frames fit.
Increasing a budget or changing model, chunk, geometry, managed package or
runtime library identity requires a new exact plan; it cannot silently adopt
old output. Conditional throughput remains a separate observation from exact
accepted provider durations in [recovery progress](recovery-progress.md).
