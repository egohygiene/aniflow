# Prepare selected core extraction and assembly dependencies

The existing inert media setup now maps `frame_extraction` and
`clean_video_assembly` to explicit `ffmpeg_dependency_id` and
`ffprobe_dependency_id` roles. It reads current local dependency identities and
supplied digest-bound build observations without source media, native probes,
model files, installation, registration or configuration writes.

Extraction uses closed `FrameExtractionSetupOptions`: existing native
`source_selection`, `timeout_milliseconds`, `maximum_capture_bytes`,
`maximum_source_bytes`, `maximum_frame_bytes`, `maximum_total_frame_bytes`,
`maximum_pixels`, `maximum_frames`, `maximum_working_bytes`, `minimum_free_bytes`.
It returns the existing extraction configuration with the two selected pins.
Assembly uses the existing complete `FinishingMediaOptions`, returns existing
assembly configuration without delivery metadata, and requires the actual
FFV1 RGB8 master plus H.264 delivery/full-frame hash/decode prerequisites. AAC
packets are preserved by that owner; no AAC encoder is needed. Source-specific
media admissibility and runtime acceptance remain in extraction/assembly.

Use one `aniflow.toolchain.core-media-setup/v1` request with a shared explicit
`profile`/`inventory` and 1..2 distinct `selections` (`capability_id`, typed
`selection`). Each core role can appear once. The profile dependencies must
exactly equal its selected executable roles with the existing filesystem,
environment-read and subprocess effects. No neural backend/model scale applies.
Publishing preparation keeps its existing separate selected scope.

```bash
aniflow --output json toolchain prepare-core --configuration core-setup.json
```

The SDK `prepare_core_media_setup` and CLI return per-step configurations,
attributed diagnostics and a combined `ready` flag. Inspect that flag even when
the report command succeeds. Every selected prerequisite must be ready; missing
files/features produce no configuration for that step. Each `configuration.json`
is scoped to its own step and returned inertly. Request/report bounds remain
one/eight MiB. Ready is supplied-observation/local-identity consistency, never
source, native provider/runtime/platform qualification or execution authority.

Six SDK cases and one CLI parser case are authored, uncompiled/unrun. Tests,
compiler/build/dependencies/lint/format/schema/docs/corpus/product/native/media/
provider/model/CI execution remains deferred to flow #88, scheduled last with
its original 45-ticket ledger. Known alignment cancellation/start-marker failure
remains unresolved, cause unproven. No private Akashic work, actual review,
publishing/release or ADR ratification occurred.
