# Local AMV notes source checkpoint — November 10, 2026

Relates to feature #116. Adds a concrete explicitly selected local Ollama
adapter using only the existing registered provider runtime. SDK and thin CLI
consume current normalized final-video evidence and return separately cited
notes, private run evidence and a content-free telemetry projection. No raw
media, source locators or metadata/lyrics text enter the model prompt.

Eight SDK, one parser and four fake-service cases are authored/uncompiled/unrun.
Manual source review covered API constructors, semantic/deployment identities,
network-versus-offline SDK meaning, current-source checks, citation refusal,
process-event accounting, fixed output membership and bounded private I/O.
This review does not establish compilation, compatibility or backend usefulness.
No tests/compiler/build, dependency/format/lint/schema/naming/docs/native/provider/
model/media/CLI/SDK or hosted CI operation ran. No downloads or paid requests.

Feature #116 and proposed ADR-0022 remain open/proposed. Native backend, actual
weights, hardware, offline isolation and sampled vision remain unqualified.
Flow #88 stays last and its original ledger is unchanged. The known alignment
cancellation/start-marker failure remains unresolved, cause unproven. No actual
human approval, source mutation/disposal, signing, publishing or release follows.

Final manual source review corrected the provider role to the existing artifact-
validator contract. The lifecycle-observer role correctly forbids outputs/writes;
its generic constraints remain unchanged. The authored manifest case now asserts
that role. No compiler, test, validator or provider operation ran.
