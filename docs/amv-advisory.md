# Evidence-only AMV advisory (#116)

`amv_advisory::prepare` and `validate_candidate`, with thin `video advisory prepare|notes` delivery, project current accepted final-video records and check supplied candidate notes. The supplied-candidate path invokes no model. The separately selected local-runtime extension below adds invocation/reporting; sampled vision and actual model usefulness remain later #116 work. Ordinary processing is unchanged when unused or disabled. There is no Flow/sibling source dependency or second provider runtime.

A private closed `aniflow.amv-advisory-request/v1` selects `mode` (`disabled` or `evidence_only`), `video_run_directory` and one `artifact_id`. Disabled preparation reads no source/model and returns no packet. Enabled preparation uses the existing final-video observer before/after projection; its current native checkpoints, exact final file and acceptance remain authoritative. This entry starts no decode, extraction, sampling or provider operation.

The portable packet contains current context/artifact/checkpoint hashes, normalized facts and explicit omissions. Geometry, frame count/rate/interval, bytes and recorded audio presence/interval/channels/sample rate cite exact report/manifest digests. Private locators, filenames, lyrics and metadata text are omitted. Rational citations refer to the selected FINAL VIDEO clock: ordinal/rate plus the recorded interval start. Original-source mapping across processing is not inferred.

`video advisory prepare --request /private/request.json` returns a preparation envelope. Save its `packet` field for candidate input. `video advisory notes --request /private/request.json --packet /private/packet.json --candidate /private/candidate.json` returns a separate ordinary machine-envelope sidecar. There are no default file writes or overwrites. SDK callers retain artifacts explicitly. A saved self-consistent packet is insufficient without fresh actual acceptance through the selected owner.

Candidates declare profile/template/output-schema pins and provider/model/runtime names. The output-schema pin must match this candidate schema's exact bytes. These declarations do not prove model execution, weight identity or suite adoption. Input is at most64KiB; each of at most16 suggestions and16 questions has1..1024 UTF-8 bytes and1..8 distinct known fact references. Frame references bind both ordinal and exact final rational PTS. Stale contexts, missing/duplicate/range/schema errors refuse. There is no retry, implicit install, upload or paid fallback.

Observed record findings are fact IDs rendered as the host's exact typed values; model-authored claims cannot enter that section. Suggestions/questions remain untrusted text. Citation membership is checked separately from semantic grounding: relevance and truth of suggestions remain UNVERIFIED. Prompt-like strings cannot become commands, paths, options or recipe edits. UIs must display notes as plain text, without executing HTML/shell/Markdown links. Local sidecars are not portable telemetry; no raw model text is exported.

Reports explicitly state zero media samples, no full-movie review, no observed provider execution, and no technical/creative/publication authority. Notes cannot establish decoding, frame integrity, A/V sync, watermark absence, perceptual quality or safety. Native validators and exact-candidate human review remain separate. This checkpoint creates no approved review record.

Seven SDK and one CLI cases are authored/uncompiled/unrun, covering disabled/missing actual acceptance, prompt-like text, invented citations, exact final PTS, schema and changed identity. No media/model was processed. Full execution and native/visual/backend qualification remain deferred to flow #88 last. ADR-0022 remains proposed; #116 stays open.

## Explicit local runtime checkpoint — November 10, 2026

`amv_advisory::runtime::run` and `video advisory run-local --request ABS
--configuration ABS --host ABS --output-directory ABS` add one explicitly
selected local Ollama notes call. The bundled standard-library Python adapter
runs through the existing registered provider runner with direct argv, exact
interpreter/configuration/adapter locks, bounded capture/output, process-group
cancellation and timeout. No second process runtime or sibling dependency is
introduced. The output root must be new, private and separate from the accepted
source. Failed roots are retained; there is no automatic retry or loose-output
adoption. A disabled request skips configuration/host/source I/O. A disabled
configuration skips host/provider/source admission. Missing selected interpreter
or resources reports unavailable without substituting another provider.

The private closed configuration example is disabled and contains placeholders,
not a tested model. Explicit host observations use the existing `HostResources`
contract. Literal loopback plus a nonzero port is required; remote endpoints,
implicit installs/pulls/downloads, paid fallback, tools, frame/audio samples and
model substitution are refused. The caller must prepare/start the selected local
service separately. The adapter checks `/api/version` and the selected tag's
full model digest before and after exactly one nonstreaming `/api/chat` request.
Only normalized packet facts go into the prompt. Host citation validation and
current source acceptance are rechecked before returning notes.

Loopback is still networking in the generic provider SDK: `offline:false` and
an explicit network grant are required. This does not grant remote/cloud
fallback. Service egress-disabled is a caller declaration, not enforced here.
Service-reported version/model/digest and token counts are not weight proof,
hardware readiness or offline qualification. Context/output settings are
requested server bounds; host timeout, process/capture and artifact bounds are
owned by the existing runner. There is one process attempt, at most one chat
call, no cache and unknown cost. This cannot certify arbitrary model usefulness
or prevent an independently configured service from making its own network calls.

Private `provider-lock.json`, `invocation.json`, `execution.json`, `outputs/notes.json`
and final `result.json` retain the run. The result contains a separate portable
telemetry projection: fixed state, exact hashes, process/model call counts,
latency and optional reported token counts. No raw prompt, note, configuration,
endpoint, path or child diagnostic enters that projection. The full result and
provider report are private sidecars, not telemetry. On failed/interrupted calls,
model call count and usage remain unknown rather than claiming zero.

The existing advisory v1 retains its supplied-candidate meaning, including
`provider_execution_observed:false`; the outer runtime result is the separate
process-execution evidence. It does not rewrite that older contract. Suggestions
are still untrusted, semantically unverified plain text. No native acceptance,
media mutation, creative review, approval, signing or posting follows. This path
has zero samples and does not inspect a full video. Sampled vision and selected
backend/native/hardware qualification remain feature work in #116 and flow #88.

Eight Rust SDK cases, one additional parser case and four fake-service Python
cases are authored/uncompiled/unrun. No interpreter/provider/model, tests,
compiler, CLI, schema checker, dependency operation or hosted CI was run.

The runtime manifest uses the existing `artifact_validator` role for the separate
notes artifact contract. It cannot use `lifecycle_observer`: that SDK role must
produce no artifacts and permit no filesystem writes. No generic role constraint
was relaxed. This adapter role does not authorize media technical acceptance or
promote a model response to a native validation report.
