# Prepare one local posting package

Accepted [lossless audio exports](lossless-audio-export.md) retain their actual
format: selected FLAC uses `public/audio.flac`, while WAV uses `public/audio.wav`.
The same current preflight, metadata-scope policy and exact supplied review apply.

The `destination::posting` SDK and `delivery posting-plan|package|package-status`
CLI prepare a separate copy of one accepted file for one explicitly chosen
manual destination. Source/master bytes remain untouched. No media encoding,
resize, crop, rate/precision/gain change, model, reference fetch, platform call
or post is performed. A mismatched preflight cannot become an export by copying.

Create closed `aniflow.posting-request/v1` JSON with the complete saved
[preflight result](destination-preflight.md), explicit `public_metadata`,
`metadata_scope_policy`, `maximum_package_bytes` and `minimum_free_bytes`.
Request/evidence JSON is bounded to 2 MiB. Public metadata requires title,
creator, release ID/version and credits; optional description, artwork HTTPS
references and provenance HTTPS reference are supplied separately. Nothing is
inherited from private native reports, local paths or requirement-checker names.
References are never fetched, and a provenance pointer is not a verified proof.

Metadata policy is a meaningful explicit choice:

- `require_clean_within_declared_scope` requires an accepted non-preserve scoped
  sanitation result. Opaque/signature/signal exclusions remain.
- `acknowledge_inherited_or_uninspected_scope` permits copying media with
  retained/uninspected metadata under the exact supplied review. It never says
  the media is sanitized or credentials remain valid.

Residual/failed metadata inspections refuse either path. Original PCM and video
without a qualifying metadata receipt need the explicit acknowledgment or a
separately accepted scoped candidate. No source metadata is silently stripped.

```bash
aniflow --output json delivery posting-plan --request posting-request.json
```

The read-only plan rechecks current native acceptance and supplied requirements,
then returns the exact `review_context_sha256` and `review_subject_sha256`.
The context binds the complete request, selected file, public fields, profile
and metadata scope; the subject is the exact media digest. Inspect the selected
media and intended public metadata yourself. Prepare a separate explicit
`aniflow.manual-review/v1` declaration containing those hashes, your decision,
reviewer and notes. A matching supplied `approve` is needed to make the copy;
rejection/wrong context never creates a package. No declaration is generated.

```bash
aniflow --output json delivery package \
  --request posting-request.json --declaration operator-decision.json \
  --output-directory "/absolute/new posting package"
aniflow --output json delivery package-status \
  --directory "/absolute/new posting package"
```

The initial writer requires Unix private permissions, a normalized fresh root
under an existing canonical parent, finite available storage and disjoint source/
accepted-run paths. It creates root/directories mode 0700 and evidence/copies
mode 0600. The fixed layout is:

```text
public/delivery.mp4   # or audio.wav, selected from the accepted media kind
public/release.json   # explicit allowlisted fields and scope/authority limits
private/request.json # complete private preflight and request
private/review.json  # exact supplied declaration
package.json         # private completion-last receipt
```

Only deliberately selected public members belong in a later posting handoff.
Do not publish the private directory or root receipt. The source media may still
contain acknowledged inherited/unsupported metadata; the public JSON never
copies private dictionaries. No source authentication, creator identity,
ownership, actual publication or timestamp proof is inferred from the declaration.

Pinned copies, hashes, current producer/preflight rechecks and exact member
inventory precede the last receipt. `package-status` verifies the request/review,
receipt, all member bytes/membership and current producer authority. Missing
original evidence refuses current readiness while retaining package bytes.
Changed source/policy/public fields require new binding/review. Partial writes
stay unaccepted and preserved; no loose file, retry, existing folder or stale
writer is automatically adopted, replaced or cleaned up. Use a fresh root after
explicit reconciliation. Exact-byte archival verification remains separate.

Envelopes use `delivery_posting_plan`, `delivery_package` and
`delivery_package_status`. Seven binding/privacy/refusal/inventory cases and one
CLI parser case are authored, uncompiled/unrun. Tests/builds/validators/native/
CLI/SDK/provider/media/model/CI remain deferred to
[flow #88](https://github.com/egohygiene/flow/issues/88). The alignment cancellation/
start-marker failure remains unresolved, cause unproven. No package or human
review was actually created in this source sprint. Actual conversion/other
formats, platform/post-upload and full #83/#82 feature acceptance remain open.

## Optional later supplied-copy observation

The [post-upload observation owner](post-upload-observation.md) can retain a
separately obtained local copy and operator-declared posting history in another
private root. It rechecks this exact package, observes byte equality separately
from quality notes, and performs no fetch, upload, post or media validation.
