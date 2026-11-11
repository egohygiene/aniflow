# Inspect OpenTimestamps commitments without chain trust

`release_evidence::ots::inspect` inspects supplied `.ots` bytes against an
explicit caller-selected SHA-256. This first #82 checkpoint is a pure library
operation: no file/process/network/clock access, installation, media operation,
signing or timestamp submission. A later retained-pair adapter can observe the
actual subject; this pure API does not establish that the caller's digest came
from any particular file.

The closed [report](contracts/ots-proof-inspection-v1.schema.json) distinguishes
`parsed` from `unsupported`, and `matches_expected_sha256` from `mismatch` or
`not_assessed_unsupported`. A parsed detached header plus evaluated operations
establishes local hash-commitment relationships only. Anyone can construct such
a document. Bitcoin height and Merkle-root fields are **claims**, not verified
blocks, timestamps, a chain selection, ownership or authenticated calendar history.
`timestamp_state` always stays `not_verified`; chain verification stays unperformed.

## Closed initial profile

The parser admits OTS detached v1 with SHA-256 file digests and SHA-256, append,
prepend, reverse and lowercase hexlify commitment operations. Every supported
operation is actually calculated from its parent message. Siblings start from
the same parent rather than chaining through one another. The header, recursive
branch markers, operation arguments and attestation payloads must be complete,
and no trailing bytes are accepted. Duplicate same-node operations or exact
attestations, nonminimal varuints and empty pending URIs are refused by this
deliberately stricter profile.

Recognized pending-calendar payloads retain only URI digest and size in the
report. The URI is never followed or emitted. Bitcoin claims require a uint32
height and a 32-byte calculated commitment. Unknown attestation tags/payloads
remain opaque digests and counts. Other file-hash or commitment operations and
major versions produce `unsupported` with **no partial operation or attestation
results**; unread trailing structure is not represented as parsed. Malformed,
excessive or cancelled input returns a refusal. Unsupported does not mean the
proof is universally invalid.

Admission is at most 1 MiB proof bytes, 4,096-byte messages/arguments, depth 64,
4,096 operations, 128 attestations, 64 entries per node, 8,192-byte attestation
payloads and 32 MiB of operation input/result work. Cancellation is observed at
entry and throughout traversal. These are parser bounds, not backend/platform
qualification. Operation-path identity starts with the domain
`aniflow.ots-operation-path/v1` plus a NUL byte; each operation appends its tag,
argument byte length as uint64 big endian and exact argument bytes before the
path SHA-256 is finalized. This is Aniflow correlation, not an OTS wire field.

The report contains exact proof/subject digests and untrusted claim observations;
treat it as private evidence until an explicit disclosure policy selects fields.
Raw URI/payload omission alone does not make every proof hash or Bitcoin root
appropriate for publication. This inspector does not rewrite an existing opaque
candidate receipt, upgrade a proof, verify headers or manufacture native media
acceptance. The original exact proof and subject remain necessary for later
independent timestamp verification.

## Protocol review and remaining qualification

Wire layout and operation/payload semantics were read from
[python-opentimestamps at 8c3dc6f](https://github.com/opentimestamps/python-opentimestamps/tree/8c3dc6fbdf6fcba054e78643e9df5eccd8c9fce4):
[timestamp](https://github.com/opentimestamps/python-opentimestamps/blob/8c3dc6fbdf6fcba054e78643e9df5eccd8c9fce4/opentimestamps/core/timestamp.py),
[operations](https://github.com/opentimestamps/python-opentimestamps/blob/8c3dc6fbdf6fcba054e78643e9df5eccd8c9fce4/opentimestamps/core/op.py),
[attestations](https://github.com/opentimestamps/python-opentimestamps/blob/8c3dc6fbdf6fcba054e78643e9df5eccd8c9fce4/opentimestamps/core/notary.py),
and [serialization](https://github.com/opentimestamps/python-opentimestamps/blob/8c3dc6fbdf6fcba054e78643e9df5eccd8c9fce4/opentimestamps/core/serialize.py).
No upstream implementation was installed or executed. Ten synthetic binary
fixture cases are authored **uncompiled/unrun**; a mixed-operation expected
digest was authored independently with Python hashlib without importing product
code. Actual interoperability, malformed/fault corpus and security/cryptographic
qualification remain deferred to flow #88, scheduled last. Stamp/pending/
upgrade/independent Bitcoin verification and the broader #82 acceptance remain
open. The proposed ADR-0015 extension is not ratified.
