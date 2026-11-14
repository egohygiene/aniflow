# aniflow #212 — conditional schema checker checkpoint

Base main: `e6f3cd4d272d42f63aef034aeac25c22c3567202`.
The current session authorizes this focused Python checker repair despite the
older implementation-sprint deferral in the retained source handoff.

The existing checker rejected three toolchain-probe fields and the provider-v2
failure field even though their enclosing objects declare them. Its recursive
walk compared a conditional refinement only with that refinement's own
`properties`, losing the surrounding same-instance declarations.

The walker now visits schema-valued keywords explicitly. Applicators inherit
ancestor declarations through immutable scopes; properties, array members and
definitions reset scope. A branch cannot donate names to siblings (including
`allOf` siblings) or enclosing schemas. Literal values and annotation data are
not schema nodes. Bare
`required` refinements retain their existing reference-compatible behavior;
the declaration check applies where a node has its own `properties` mapping.
Duplicate names, unresolved local definitions and malformed patterns still
fail. Malformed `required` and `properties` shapes produce diagnostics rather
than an unhashable-value crash or silent omission.

The synthetic unittest cases cover both published conditional reports,
same-instance inheritance, nested scopes, sibling/conditional isolation,
schema-keyword property names, annotation data, boolean leaves and negative
mutations of the real contracts. `task contracts:check` runs those regressions
before the existing checker. No published schema or example bytes change.

## Focused evidence

The unmodified checker reproduced the four reported undeclared-field errors.
The new regression suite exposed the inherited-scope, annotation and malformed
input problems before the repair. After repair:

- 15 checker unittest cases pass, including boundary subcases.
- The public checker validates 375 JSON contract documents.
- The existing toolchain-probe schema fixture passes all six tests.

The review PR records the exact tested head and runtime versions. This is a
structural checker fix, not a general JSON Schema resolver or proof of the
application's compilation, native providers, models, media, hosted CI or release
readiness. The separately retained compiler, naming, alignment-cancellation and
full qualification work remains under #212 and flow #88. No dependency or lock
change, hosted dispatch or publication belongs to this checkpoint.
