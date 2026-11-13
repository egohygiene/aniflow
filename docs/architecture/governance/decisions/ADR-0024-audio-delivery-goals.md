---
schema: aether.architecture-decision/v1
id: aniflow-adr-0024
title: Keep selected audio delivery goals separate from measurement and approval
kind: architecture-decision
status: proposed
owners: [egohygiene]
scope: [aniflow]
governed_by: [architecture-decisions]
supersedes: []
superseded_by: []
related: [aniflow-architecture, aniflow-adr-0019]
---

# ADR-0024 — Explicit audio goals and candidate review

Under #212's retained #83 scope, goals are explicit versioned operator choices,
not inferred universal platform standards. The library owns a closed numeric
policy and its identity. Current exact-final-file QC owns observed levels;
comparison does not upgrade the meter's qualification or supply creative review.

Any selected adjustment creates a separate candidate with exact source, policy,
configuration and component bindings. Original masters and evidence remain
immutable. Processing, fresh final-byte QC, policy comparison and human review
remain distinct; predicted gain/peak behavior never replaces measured output.
Unsupported goals must remain visible rather than invoke an implicit limiter,
resample, downmix or different runtime. Native v3 remains acceptance authority.
Flow owns cross-tool project selection and publishing coordination with no
sibling source dependency. Nothing grants upload or release authority.

This is an unratified source proposal. Native/reference-meter, codec, temporal,
resource and real musical qualification remain deferred. Flow #88 stays last;
the known alignment cancellation/start-marker failure has unproven cause.

## Proposed machine handoff extension — November 12, 2026

The delivery facade delegates selected native requests to the existing owners
and returns a versioned plan and accepted handoff. Source files, full native
request/intent, exact artifacts and limited output state enter canonical hashes.
Status performs current owner readback; explicit QC resume first compares the
supplied request with the retained native request. No parallel state engine or
automatic retry, adjustment, goal choice, human review or publication is added.
Original native commands remain independently usable. Flow consumes this public
boundary without a sibling dependency. This extension remains proposed, and
native/codec/reference-meter qualification remains unexecuted.
