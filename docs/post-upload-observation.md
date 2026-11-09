# Retain a supplied platform-copy observation [skip ci]

This optional workflow takes an already obtained local file and an exact
reviewed posting package. It never fetches, uploads, posts, logs into an account
or downloads media. A supplied URL or filename does not authenticate a platform.

The SDK owner is `destination::post_upload::{retain,inspect,public_summary}`.
Thin CLI delivery is:

```sh
aniflow delivery post-upload-record --request private-observation.json --output-directory /absolute/new-private-root
aniflow delivery post-upload-inspect --directory /absolute/retained-root --receipt-sha256 <KNOWN-RECEIPT-SHA256>
```

These commands are documented only and were not run. Machine-envelope commands
are `delivery_post_upload_record` and `delivery_post_upload_inspect`, with data
schema `aniflow.post-upload-observation/v1`. The request schema is
`aniflow.post-upload-observation-request/v1`.

Supply the exact posting-package directory/receipt digest, matching destination,
local candidate file/digest, finite copy/free-space limits and an explicit
publication declaration. URL, optional posting time, observation time, observer,
acquisition history and picture/audio-stereo/timing/metadata/watermark check
notes are operator declarations. Their chronology and bounds are checked, not
their truth. Concerns need a reason. Missing review remains `not_reviewed`;
unavailable detector results do not become successful watermark verification.
URLs reject credentials, query/fragment data, controls and malformed hostnames.
They are retained privately and never fetched or exposed by the public summary.

Retention first rechecks current exact posting/package producer authority. It
copies the nonempty selected file into a fresh disjoint Unix-private root,
retains the private request and posting snapshot, rechecks selections, and
writes `observation.json` last. Copy admission is at most 1 GiB; hashing/copying
is bounded by selected bytes, with existing streaming I/O and cancellation
boundaries. No stronger deadline or process-resource enforcement is claimed.
Source package, masters and existing observations remain unchanged. Partial or
unknown roots refuse and remain for explicit reconciliation.

Byte comparison reports `identical` or `different` independently of quality
notes. A changed platform copy may be a normal transcode; it is not automatically
bad or accepted. No decode, frame/sample/timing/color/loudness/metadata/watermark
verification is performed here. Platform policy, actual origin, publication and
quality remain unverified, including when bytes match exactly.

Archival inspect requires a separately retained receipt SHA-256, checks strict
membership and exact copied bytes, and can work after original local paths are
unavailable. It does not reassert current producer acceptance. The machine
result includes private declarations; keep it out of portable telemetry. Only
an explicitly requested `public_summary` omits URL, paths, observer, supplied
times and notes; sharing it remains a separate decision.

Seven SDK cases and one parser case are authored, uncompiled/unrun. Tests,
builds, checks, native/media/provider/model/CLI/SDK/CI execution remain deferred
to flow #88, scheduled last. #83 retains actual additional encoders, current
platform policies and all original media/creative/platform qualification.
ADR-0013 remains proposed. Known alignment cancellation/start-marker failure
remains unresolved; cause unproven. No actual content review or post occurred.
