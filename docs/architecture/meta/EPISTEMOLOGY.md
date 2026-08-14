---
schema: aether.architecture-document/v1
id: aniflow-epistemology
title: aniflow Epistemology
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-epistemology
depends_on:
  - aniflow-purpose
  - aniflow-principles
related:
  - aniflow-ontology
  - aniflow-decisions
supersedes: []
---

# aniflow Epistemology

## Purpose and scope

This document defines how aniflow distinguishes declarations, observations,
inferences, decisions, and uncertainty when inspecting media and reporting a
run. It governs evidence quality, not the outcome a processor should produce.

## Claim states

| State | Meaning in aniflow |
| --- | --- |
| Observed | Directly returned by a recorded probe, filesystem read, process, or validator |
| Declared | Supplied by a user, pipeline, processor contract, or external manifest |
| Inferred | Derived from observations using a named rule or calculation |
| Decided | Selected execution or acceptance behavior; not a fact about the source |
| Proposed | Suggested contract or behavior not yet accepted |
| Assumed | Temporarily treated as true so work can proceed |
| Unverified | Present but not evaluated sufficiently |
| Disputed | Credible observations or interpretations conflict |
| Deprecated | Retained for compatibility but no longer preferred |
| Open question | Intentionally unresolved |

## Evidence sources and evaluation

Relevant sources include source bytes, cryptographic digests, media probes,
decoder and encoder observations, filesystem metadata, processor declarations,
tool versions, command results, raw logs, validators, fixtures, and human review.

Evidence is evaluated by directness, reproducibility, provenance completeness,
tool and method identity, transformation history, applicability, recency, and
consistency with independent observations. A tool's authority or zero exit
status does not by itself prove the semantic claim being evaluated.

## Provenance expectations

Every material observation records its source, method, version when relevant,
time, scope, and relationship to the artifact. Normalized facts retain a link
to raw evidence. Missing provenance is labeled; it is never reconstructed from
guesswork.

aniflow run evidence is operational provenance about its own processing. It is
not automatically a C2PA claim, origin attestation, authorship proof, or suite-
wide authenticity judgment.

## Confidence and uncertainty

Use `high`, `moderate`, `low`, or `unknown` only when a conclusion is inferred
rather than directly observed.

- **High:** repeatable, direct evidence with understood method and no material
  conflict.
- **Moderate:** relevant evidence supports the claim but a known limitation or
  dependent inference remains.
- **Low:** evidence is indirect, incomplete, or materially method-dependent.
- **Unknown:** support has not been evaluated or provenance is insufficient.

Uncertainty records what is unknown, why, its execution impact, and which
observation could change the conclusion. Numerical confidence is avoided unless
the producing method is calibrated and documented.

## Conflict resolution

When credible evidence conflicts, preserve each observation and provenance,
identify the exact disagreement, separate factual from interpretive conflict,
and record the working decision independently. A safe run may reject or require
explicit policy without pretending the conflict has been resolved.

## Canonical working knowledge

A claim becomes canonical for a run only when its scope, evidence, method, and
applicable validation satisfy the versioned contract. Repository-level
knowledge becomes canonical through reviewed documentation, tests, or accepted
decisions. New evidence revises or supersedes the claim without erasing its
history.

## Human and AI contributions

Human review and AI-generated analysis are inputs whose provenance and method
must remain visible. Neither is self-validating. Private reasoning is not
required evidence; reviewable observations, rules, fixtures, and decisions are.

## Assumptions and open questions

Perceptual quality and continuity metrics will require method-specific
confidence models. Until calibrated, they must report observations and limits
rather than binary truth.

## Validation

Fixtures test incomplete, contradictory, and transformed evidence. Public
results keep claim state and provenance distinguishable.
