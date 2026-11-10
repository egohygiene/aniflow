# Inspect a recovered recipe before release

The #107 source checkpoint extends the fixed release-stage journey to existing
recovered v1 and optional-RIFE v2 recipe owners. It does not change the
source/indexed `entry-status` contract or create another workflow executor.

```bash
aniflow --output json video stages recovered-status \
  --request "/private/recovered entry.json"
```

Use `aniflow.release-stages-recovered-entry-request/v1` with explicit
`recipe_kind` (`recovered_v1` or `recovered_with_rife_v2`), existing normalized
`recipe_run_directory`, exact `expected_recipe_intent_sha256`, selected
`artifact_id` and optional `release_request`. The
[synthetic request](../examples/music-video/recovered-stage-entry.synthetic.json)
is inert; its locators and digest are not actual native evidence.

The existing recovered owner reobserves source/index, accepted origins, native
settings/components, completed carrier and native children. The view binds that
owner's complete status digest, projects reached child plan/manifest identities
and distinguishes failed, cancelled, running and interrupted work. Inspection
advice never resumes, imports or adopts loose outputs. Pending advice names the
existing explicit recipe resume operation; it grants no execution authority.
Per-step ordinal counts supplement the bound owner status, which remains the
source of the exact ordinal list and full recovery details.

Omit the release request until actual native assembly accepts the selected
master. Only a current complete parent, all accepted steps, exact final-video
handoff and unique selected output can reach the unchanged release tail. Its
`accepted_video` source must name this same recovered root/output and actual
current SHA-256. A different master, source root, stale hash or missing parent
cannot substitute. Later title/lyrics, finishing, final-file review and package
owners keep their current requirements. Imported-frame decisions do not approve
the complete movie, and this view never supplies a creative decision.

The projection is reobserved twice and context-bound. Request and output limits
are 512 KiB and 2 MiB; larger projections refuse without truncation. The existing
native source/current-state calls keep their own cancellation and I/O limits;
this source checkpoint is not native interruption or workflow qualification.
Six library/refusal/projection cases and one CLI parser case are authored,
uncompiled/unrun. No tests/builds/native/provider/model/media/CLI/SDK/CI execution,
actual review, source replacement/disposal, posting or release occurred.
ADR-0023 remains proposed; #107 acceptance stays open and Flow #88 stays last
with its original ledger untouched. The alignment cancellation/start-marker
failure remains unresolved and its cause unproven.
