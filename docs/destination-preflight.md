# Check a selected destination against accepted media

`destination::preflight` and `delivery preflight --request request.json` read
current accepted media and compare explicitly supplied requirements. No media
tool, platform API, transformation, upload or posting is invoked. The result is
`matches_supplied_profile`, `needs_changes` or `missing_evidence`; none means a
service accepted the file or a person approved its content.

Inputs are closed `aniflow.destination-preflight-request/v1` JSON, at most 64 KiB:

- `accepted_video`: canonical `run_directory` plus exact `artifact_id` from
  `video completion`. Existing assembly/recipe/remux acceptance is rechecked.
- `accepted_pcm`: canonical original `file` plus `signal_run_directory` whose
  current accepted source is those exact WAV bytes. Independent native WAV facts
  must agree with its clock/channels/sample count. Stem sources are refused.

The first profile kinds are manual TikTok fixed H.264/yuv420p BT.709 limited-range
MP4 destination copies and manual SoundCloud/DistroKid PCM WAV. WAV preserves
the admitted PCM16/packed PCM24/float32 representation; no FLAC encoder, decode,
precision reduction, dither, rate conversion, gain or channel change is added.
Other native formats need their own accepted observation/transform profile.

Supply `aniflow.destination-profile/v1` with an ID/revision, destination/manual
route, finite file/duration/media requirements and `evidence`: official HTTPS
source URLs, `checked_on`, `valid_through` and `checked_by`. These are supplied
declarations, not verified research. Profile URLs are never fetched. An explicit
`evaluated_on` outside that interval yields missing evidence; replaying an old
request does not refresh current platform rules. Recheck route/account/current
requirements before actual delivery. Source URLs alone do not establish that
the numbers were correctly reviewed.

Video checks cover fixed supported format, actual axes and optional exact
geometry, exact rational rate, selected audio/channels, optional zero origins
and matching picture/song extent. Duration uses the entire picture/song union,
including offsets/tails; it never trims a song or suggests success after silent
reframing. PCM checks cover exact original rate/channels/encoding and duration.
Requested loudness requires accepted exact-subject integrated/true-peak values.
Unavailable is not zero; video loudness cannot be inferred from unrelated PCM.
No universal loudness target or musical improvement is invented.

```bash
aniflow --output json delivery preflight --request request.json
```

The private `aniflow.destination-preflight/v1` retains request/profile identity,
actual accepted evidence/file facts, per-requirement reasons and a payload digest.
Save the envelope's `result`, not the envelope itself, when another operation
needs the report. `destination::recheck` recomputes its current accepted facts
and compares the exact saved request, requirements and report. A serialized or
rehashed invented observation cannot replace native acceptance. Keep the full
report out of public posting bundles; it contains private locators/lineage.

[Synthetic request](../examples/destination/preflight.synthetic.json) illustrates
shape only. Its old requirement dates deliberately need refresh and its paths
are placeholders. It is not a current destination preset or an approved release.
No provider or platform requirement was independently qualified in this sprint.

Six comparison/refusal cases and one CLI parser case are authored, uncompiled/
unrun. Tests/builds/dependencies/lint/format/validators/CLI/SDK/native/provider/
media/model/CI remain deferred to [flow #88](https://github.com/egohygiene/flow/issues/88).
The alignment cancellation/start-marker failure remains unresolved/cause unproven.
Actual destination encoding, other formats, full measurement/export/post-upload
and timestamp/credential lifecycles remain in #83/#82 and their feature owners.
