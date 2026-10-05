# aniflow #104 implementation checkpoint

Parent: [#38](https://github.com/egohygiene/aniflow/issues/38).
Base: `08cb2a83dbe288f6d893a278cbeb34905cc19a49`, merged PR #100,
tree `999fc023238d69d2c24423dd263158c111d8f251`.

## Bounded outcome

Prepare explicit native-v2 audio-inspection registration documents from a
reviewed profile/inventory, one stage binding, a separately prepared adapter
and a supplied source identity. The public library derives matching tool pins,
component inventory and provider settings. The thin CLI command is
`toolchain prepare-registration --configuration "request.json"`.

Successful preparation returns four inert JSON file contents: registration,
manifest, effective provider configuration and toolchain preflight. It writes
no files, installs or downloads nothing, launches no tool and registers no
provider. A not-ready result retains diagnostics and inventory facts with no
generated files; the CLI retains this result in its machine error envelope.

The caller must explicitly review and place the generated documents in the
declared registration directory with its existing compatible adapter, without
overwriting unrelated files, then supply registration and preflight to Pipeline
v3. Preparation does not read or decode source media. The source digest/size
remains a declaration for subsequent source-binding checks.

## Identity limits

Physical tool/adapter hashes describe local observations during preparation.
Version, package and feature observations remain attributed caller evidence;
no package, complete environment or native qualification is inferred. The
adapter must independently implement the native invocation ABI. Hashing a file
cannot establish that behavior.

Existing registration/lock contracts are unchanged. Registration v1 cannot pin
the preparation-time adapter digest for later initial planning. Before adopting
the first plan, compare its locked implementation digest with the prepared
adapter observation; re-prepare and review on drift. Existing exact lock and
preflight checks govern run/resume after that plan is adopted. The output is not
an execution authorization or an atomic executed-file attestation.

## Deferred validation

Focused synthetic Rust and Python schema coverage is authored, including inert
document integration with the existing explicit registration/preflight path and
refusal cases. All coverage remains **unrun under #64**. No tests, compiler/build,
formatting/lint, schema/drift, smoke/package, native platform/tool, or hosted CI
execution is claimed. Source review is limited evidence only.

The known alignment cancellation failure remains unchanged:
`cancellation_keeps_technical_checkpoint_and_resume_reexecutes_alignment`,
`tests/audio_alignment.rs:547` (`marker.exists()`, joining thread at 561), cause
unproven. Historical receipts remain intact. Cargo remains `0.3.0`.

## Remaining work

Parent #38 remains open for complete package/runtime/environment identity,
additional typed mappings, optional tool/device probes, model locators and
actual platform qualification. Registration materialization is still explicit
caller work. Execution qualification stays with #64 and actual immutable
publication with #10. The next media implementation checkpoint is #36
(frame-workspace import and selective repair), followed by #37, #35, #83 and #82.
