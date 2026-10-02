# #69 implementation handoff

## Delivered source

Base: merged main `be2833185c0737e5e955e5a34cabd773bc6d3f5f`, tree
`e394fdb5dc6f063d31fe5459a091f0ca196af731`, including PR #88 release tooling and
PR #84 manual media guidance. The retained local checkout had exactly that tree.
Implementation checkpoint: `03eadfb4f0fbd30aa37f4ed736ea369ac4447218`, tree
`a5e2779a1369d57ac195c366a18dc100760738b2`. The final candidate also contains
this handoff, its implementation receipt and refreshed root continuity.
The PR records the final published head/tree; Git transport remains unavailable.

- #89 versions exact artifact-set invocation/report identity and closed schemas.
- #90 executes and accepts every planned member and binds the selected ABI into
  complete-set resume, status and cache proofs.
- #91 authors synthetic coverage, source locators and provider/consumer guidance.

## Contract and publication boundary

An all-`one` stage keeps invocation/direct-argv v1 and execution report v1.
A stage declaring `one_or_more` or `many` uses v2 even for a singleton set.
Every declared output port must have a finite, explicit, nonempty planned set;
`one` still means exactly one. Optional/unbound/empty sets and discovery stay
unsupported. Providers must explicitly handle v2 and bump their own identities
when changing behavior. The existing reference-provider bundle stays v1.

Invocation members are identified by port and unique artifact ID, with disjoint
paths and exact kinds. Reports identify sorted `(port, relative_path)` pairs;
the immutable plan maps those pairs to artifact IDs. V2 report digests cover
canonical `{schema, payload}`; v1 digests remain payload-only. V1 invocation
parsing now explicitly enforces the existing executable one-output-per-port
restriction instead of permitting an unexecutable repeated-port request.

A successful report is not stage acceptance. Every member is reobserved and
must pass its own obligations. The complete stage acceptance/checkpoint is the
commit boundary; downstream execution, delivery and cache reuse require it.
An interruption can leave unaccepted copied bytes or intermediate evidence in
the workspace. Those are not a partial accepted checkpoint or delivery, and
resume validates/reruns the complete affected set. No filesystem-wide
transaction or security sandbox is claimed.

Provider-backed validators remain on their singleton v1 profile. Validator
and producer report schemas are checked independently. The stem-import consumer
uses the shared ABI selector and complete pair identity; its invocation hash
also includes the existing acceptance semantics to match the executor.

## Coverage and qualification

Authored synthetic cases cover repeated ports, report version/digest confusion,
duplicate or overlapping paths, exact member kinds/digests/sizes and aggregate
bounds, missing/extra siblings, cancellation, complete-set resume, sibling
tampering, cache corruption, fresh-inode materialization, and failed sibling
validation preventing acceptance/cache publication. See the source list in
[the implementation receipt](aniflow-69-implementation.json).

**Unrun under #64:** tests, builds, compiler checks, formatting, lint, schemas,
corpus drift, native tools, smoke/package checks and hosted qualification.
No remote CI was dispatched or polled. Publishing the draft can trigger existing
repository events; their outcomes are unobserved. Source review and publication
tree identity are implementation evidence only.

Corpus test locators and source digests were authored directly after the source
edits. No recipe bytes changed and no corpus generator/checker was executed.
The known alignment cancellation failure remains at
`tests/audio_alignment.rs:547` (`marker.exists()`; joining thread at line 561).
Its cause is unproven; no fix or passing test is claimed.

## Resume and next work

Read AGENTS.md and root CONTINUITY.md, then re-query the PR/main and #64 before
acting. Review this draft; merge is not part of the current instruction.
#80 native PCM24/float32 inspection is the suggested next implementation issue.
#10 still needs Egolint #29, #64 qualification and an actual immutable release
before Flow #51 consumption. Broader corpus #24 and the private media pilot #79
retain their own acceptance. No version bump, tag, release, media/model run or
real source mutation occurred here.
