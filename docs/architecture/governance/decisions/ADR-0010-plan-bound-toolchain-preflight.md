---
schema: aether.architecture-decision/v1
id: aniflow-adr-0010
title: Bind typed toolchain preflight to immutable plans
kind: architecture-decision
status: proposed
owners:
  - egohygiene
scope:
  - aniflow
governed_by:
  - architecture-decisions
supersedes: []
superseded_by: []
related:
  - aniflow-architecture
  - aniflow-roadmap
  - aniflow-adr-0003
  - aniflow-adr-0004
---

# ADR-0010 — Bind typed toolchain preflight to immutable plans

## Status and authority

Proposed for #103 under #38. This draft has no recorded maintainer acceptance.
Implementation delivery does not establish ADR acceptance or qualification.

## Context

Offline inventory consistency cannot prove that a runtime adapter will use
those files or apply declared settings. An optional unbound preflight sidecar
could also be omitted during resume, bypassing an earlier readiness decision.
The provider registry and resolved lock already own provider selection and
effective configuration identity; toolchain setup must preserve that authority.

## Proposed decision

Use a separate versioned preflight document with embedded profile/inventory
and explicit stage/registration/dependency bindings. Initially map only the
existing native-v2 audio-inspection adapter's typed FFmpeg/ffprobe pins and
versions. Reject unsupported settings, backend and scale semantics. Selected
bindings do not imply coverage of other stages.

Bind the canonical document SHA-256 into optional resolved-plan v1 payload
field `toolchain_preflight_sha256`. The plan digest then carries that requirement
through immutable workspace plans and existing checkpoint authority. Guarded
run/resume require the exact document, re-resolve exact provider locks, and
freshly inspect pinned local files before input rebinding, cache use or launch.
Resume may acquire its existing writer lock first. A report's stored `ready`
flag is never accepted as execution authority.

The initial binder runs after ordinary read-only planning, including source
identity hashing. It does not prepare a registration, install tools, invoke
probes or authenticate package revisions. Observations remain attributed
declarations. Native qualification stays false.

## Compatibility and alternatives

Provider-lock v1 is unchanged. Historical plans omit the optional field and
retain their serialized bytes and digests. Strict older consumers reject the
new guarded form. Explicit null and malformed digests are invalid. Rust callers
constructing plan payload/request struct literals must account for the added
optional field; the pre-1.0 facade provides constructors/builders.

An unbound runtime flag was rejected because resume could drop it. Generic
JSON-pointer mappings were rejected because configuration hashes alone cannot
establish that profile settings are applied. Embedding the entire inventory in
every plan was rejected to keep host paths and large setup evidence outside
portable plan payloads. Guarded plans deliberately remain tied to the exact
local document digest; moving tool paths requires a new plan.

## Consequences and limits

The same guard runs even when compatible cached work might be reusable. Files
can still change after inspection; preflight is not atomic executed-file
attestation, a trust boundary for malicious programs or a process sandbox.
The adapter retains its existing execution-time tool checks. Future mappings
need typed semantic checks and their own bounded coverage; automatic discovery,
setup, probe ingestion and native qualification remain separate work.

## Evidence and validation

#103 authors strict document, identity drift, guard persistence and refusal
ordering cases using synthetic files. Source review is limited evidence.
All tests, compiler/build, formatting/lint, schema checks, smoke/package and
native/hosted qualification remain unrun under #64. No acceptance or successful
execution is inferred from this proposal.

## Registration preparation follow-on (#104)

The proposed #104 convenience boundary derives existing native-v2 audio
registration, manifest, configuration and preflight documents from one explicit
request. It returns inert file contents, never writes or registers them, and
preserves the caller's source declaration and selected stage binding. Local
tool and adapter hashes describe preparation-time observations; package,
version and source claims retain their original evidence limits.

Provider-registration v1 and provider-lock v1 stay unchanged. The registration
document cannot enforce a preparation-time adapter digest during later initial
planning. A caller must compare the first plan's actual adapter identity with
the preparation before adopting it, and re-prepare on drift. Once a plan is
adopted, existing exact locks and fresh preflight retain run/resume authority.
This follow-on remains proposed and unqualified under #64.

## Review triggers

Revisit before adding adapters, portable/relocatable guards, authenticated
package provenance, automatic setup, or any way to relax resume requirements.
