---
schema: aether.architecture-decision/v1
id: aniflow-adr-0025
title: Acquire selected managed toolchains before pinned media setup
kind: architecture-decision
status: proposed
owners: [egohygiene]
scope: [aniflow]
governed_by: [architecture-decisions]
supersedes: []
superseded_by: []
related: [aniflow-architecture, aniflow-ADR-0003, aniflow-ADR-0010]
---

# ADR-0025 — Selected managed toolchain acquisition

Issue #248 and the maintainer's selected-execution direction introduce a public
library-owned acquisition boundary before existing pinned media setup. A normal
selected run may acquire its missing curated external executable/model closure.
Read-only planning remains effect-free; offline/no-download forbids network,
while selected acquisition may assemble exact verified cached artifacts. Explicit
user paths remain authoritative and outside managed ownership. Only selected
providers are acquired; managed RIFE does not select interpolation or introduce
an automatic CPU/cloud fallback.

A source-bundled versioned catalog binds real upstream release/model/license
identities, hashes, lengths and closed extracted inventories. A dedicated
user-owned store verifies downloads, safely extracts selected regular files into
private staging and atomically publishes immutable version directories with
complete receipts. Bounded transport and extraction, cancellation, a writer
lock, closed inventory readback and durable retained-run references protect the
acquisition/lifecycle boundary. Repair preserves uncertain bytes in quarantine.
Upgrade/rollback retain old versions; uninstall accepts managed ids rather than
arbitrary paths and refuses active or retained-run references.

Resolved paths and exact package identities feed the existing inventory,
Upscayl recovery and RIFE owners. They retain process execution, checkpoint
compatibility, temporal validation and native acceptance authority. This is not
a second scheduler, dependency resolver for arbitrary software, remote-policy
loader, system package manager or release publisher. Source availability,
verified acquisition, native host/model execution, perceptual quality and release
qualification remain distinct evidence. The current catalog contains no verified
Windows/musl/Linux ARM package, and native Mac/GPU qualification stays pending
until executed on those hosts.

This source proposal records the material network and ownership extension;
implementation or feature merge alone does not ratify the ADR. Review before
adding external catalog trust, paid hosts/authentication, new archive formats,
system-wide installation, automatic provider fallback or broader lifecycle
ownership. See [managed toolchains](../../../managed-toolchains.md) and the
[acquired catalog evidence](../../../toolchain-managed-catalog-evidence.md).
