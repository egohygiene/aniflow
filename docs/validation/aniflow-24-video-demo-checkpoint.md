# #24 hands-on synthetic video demo checkpoint

The maintainer requested fake test videos that can be generated after a fresh
clone and used for local tests and hands-on exploration on October 28, 2026
(America/New_York). This bounded developer/demo slice starts from open source
recipe draft [#134](https://github.com/egohygiene/aniflow/pull/134), head
`1b09f835351800a45d570233f4981fdadf7aae99`, tree
`b34865a681ff4546ccbc751e0fe1700fe2b60ccd`. It remains stacked above extraction
draft #133. No merge or release occurs in this continuation.

Branch: `codex/24-synthetic-video-demo`. Source review is recorded below;
publication is pending. The current source package remains 0.3.0/unreleased.

## Delivered scope

- A Python-standard-library demo generator uses the existing corpus process,
  tool-identity and fresh-directory helpers. Existing corpus recipes, native
  profiles, catalog projections and qualification tiers remain unchanged.
- Two deterministic raw Y4M patterns produce 48-frame, 24-fps landscape
  320×180 and portrait 180×320 MP4s using explicitly selected local
  FFmpeg/ffprobe 8.0.1. No private media, fonts, models or downloads are needed.
- The encoded source profile is video-only H.264/yuv420p, square/unrotated,
  progressive, zero-origin CFR with declared limited-range BT.709. Bounded
  native observations belong to demo generation; they do not grant aniflow
  acceptance or establish supported native hosts.
- Matching source recipe configurations bind actual executable identities,
  disable repair/upscale explicitly and use small demo-specific media bounds.
  Existing conservative pipeline copy/report allowances still need scratch
  headroom; these settings are not intended for arbitrary user videos.
- A manifest and retained logs distinguish generation and observed file
  identity from unrun product tests and qualification. Output must be a new
  directory outside the checkout; interrupted work is retained without cleanup.
- `task demo:video` exposes generation; `task demo:video:test` exposes the small
  stdlib regression suite. Neither generation nor documentation runs aniflow,
  Cargo, tests, installers or model inference implicitly.
- The guide covers fresh checkout, clip viewing, explicit build, plan/run,
  status/resume, output comparison and separate test commands. Future local
  trial results must be recorded separately from this authored checkpoint.

## Review and execution boundary

Five focused pure regressions are authored and unrun for deterministic visible
raw frames, strict observation refusal, local tool-pinned configuration,
literal no-overwrite encoding arguments and output ownership. Root and
independent manual source review found no remaining concrete blocker after
demo-specific resource bounds and malformed-observation refusal were corrected.
The receipt records the generator source digest and Python/tool identities.
No executed result is claimed.

All generator/media execution, compiler/build/dependency resolution, tests,
lint/format, schema/naming/drift/docs validators, CLI/provider/native/model,
smoke/package and hosted qualification remain unrun under the maintainer's
#64 deferral. No Actions are dispatched or polled. Commits use disabled hooks
and `[skip ci]`. The known alignment cancellation/start-marker failure remains
unresolved, cause unproven. Documentation of commands is not an execution
receipt or authorization to process private media.

## Remaining work

At the selected hands-on qualification checkpoint, generate the pack with the
pinned tools, try both orientations through the actual built CLI, inspect
delivery output and retained run evidence, and exercise completed/interrupted
resume. Run the pure regression suite and relevant existing source-entry
tests explicitly. Record failures rather than weakening assertions or calling
successful generation complete native qualification.

#24 remains open for its broader adversarial corpus and qualification scope.
This visual demo adds no audio, neural repair/upscale, arbitrary media support,
destination profile or release. #35/#111 retain their remaining feature scope;
the proposed next feature remains #111's prior Upscayl spatial-index recipe
entry. The current draft stack and #64 qualification remain separately owned.

See [the hands-on guide](../video-demo.md), [the corpus](../adversarial-corpus.md)
and [the music-video recipe](../music-video-recipe.md).
