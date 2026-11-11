# Native contract inventory

`aniflow --output json release-contract` returns the existing machine envelope
(`command: release_contract`, `schema_version: 1`) with an
`aniflow.release-contract/v1` result. The public library entry is
`release_contract::describe()`. Neither path probes tools/models or launches work.

The result binds compiled package/library versions and source schema hashes. It
describes the initial source music-video Flow profile with exact argv templates,
native status/resume/completion commands, manual timed-text and accepted final-video
interfaces, prerequisites and source-preserving effects. Empty argv arrays mean
that the profile is inventory only, not an executable Flow route. This is an
explicitly scoped initial inventory, not an exhaustive list of native features.
Use manually supplied SRT/timing; automatic lyric recognition is not a prerequisite.

Existing native release preparation now compares descriptors from both isolated
binaries, validates their source schema bindings, retains canonical descriptor
bytes in `contracts/aniflow-release-contract-v1.json`, and records its digest in
`build.json`. Archive/bundle verification checks the same digest and represented
source files. This extends the existing preparation pipeline. It does not create
a release, execute preparation during authoring, or bypass release gates.

Linux x86-64 and Apple Silicon remain the declared native packaging targets.
Intel macOS and Windows remain explicit objectives, with no invented artifact or
native qualification. Source compatibility, published artifacts, prepared tools,
runtime availability and executed native acceptance remain separate facts under
Flow's cohort projection. No Flow dependency is added to Aniflow. Relates to #212.

Execution is deferred: authored cases and release commands have not been run in
this checkpoint. Flow #88 remains last and unchanged; the known alignment
cancellation/start-marker failure remains unresolved with cause unproven.
