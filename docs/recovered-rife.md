# Interpolate a complete recovered spatial carrier

`video_interpolation::recovered` binds the existing serial RIFE owner to an
explicit complete recovered carrier. Historical/current per-frame origins stay
in that carrier; no historical tool/model history is invented. Original source
inspection and the rational full-duration 2x clock remain separate from actual
upscaled PNG geometry and hashes. Existing original/native-spatial v1 paths
retain their serialized meanings.

Supply `aniflow.recovered-rife-request/v1`: `source` contains canonical
`carrier_directory` and externally retained `carrier_sha256` for `carrier.json`;
`configuration` is the existing explicit enabled RIFE configuration and
`provider_executable` is the selected compiled aniflow native facade. The model,
CPU/explicit device, geometry/resource and cut/hold policy remain the current
bounded profile, with at most 50000 source/100000 derived frames.

```bash
aniflow --output json video rife recovered plan --request recovered-rife.json
aniflow --output json video rife recovered run --request recovered-rife.json --run-directory /private/recovered-rife
aniflow --output json video rife recovered resume --request recovered-rife.json --run-directory /private/recovered-rife
aniflow --output json video rife recovered status --run-directory /private/recovered-rife
aniflow --output json video rife recovered bind-assembly --request recovered-rife.json --run-directory /private/recovered-rife --output /private/rife-assembly.json
```

Plan reads current carrier/origins, actual PNGs and declared components without
launching a process. Run uses a fresh fixed native v3 root. The existing shared
spatial IO/render lifecycle snapshots pinned source/tools/models and invokes
the same serial frame-pair renderer with literal argv. Ordinary v3 owns current
acceptance, failure/cancellation and compatible resume. Incomplete initialization
is retained/refused; accepted work is never adopted from loose filenames/counts.
Source/current-origin, original clock, model/configuration or PNG changes refuse
stale reuse. RAM/storage estimates remain admission, not OS/VRAM quotas.

After actual native acceptance, `observe_recovered_run`/`bind_recovered_run`
project a v2 RIFE assembly carrier containing original inspection plus recovered
lineage and actual native output. Assembly consumes that carrier through its
new v3 operation, retaining full frame/pixel/clock/decode/audio extent gates.
It creates no new parent acceptance database or inferred human approval.
`status` is ordinary read-only native state; observation/binding additionally
requires the exact current recovered origin/configuration. No source is changed.

Six SDK mapping/version/refusal cases and one CLI parser case are authored,
uncompiled/unrun. All tests/compiler/build/dependencies/lint/format/schema/docs/
corpus/product/native/provider/media/model/CI execution remains deferred to flow
#88, scheduled last with the original 45-ticket ledger. No private Akashic media,
actual review or model processing occurred. Known alignment cancellation/start-
marker failure remains unresolved, cause unproven. ADR-0020 remains proposed.
