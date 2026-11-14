# Exact final-video audio QC — November 14, 2026

This [#212](https://github.com/egohygiene/aniflow/issues/212) checkpoint follows
the accepted synthetic CPU/manual-SRT pilot. It observes audio from its exact
accepted H.264 delivery, rather than treating original source measurements as
evidence for a later file. No product change was necessary in the existing QC
or destination owners.

The locked Rust 1.85 diagnostic CLI from the
[output-file repair](CLI-OUTPUT-FILE-20261114.md) executes fresh QC plan/run/status.
The complete decoder observes 128000 stereo samples at 48 kHz, with an unchanged
8/3-second extent. Its decoded PCM SHA-256 is
`1136c3ab27749a6dd12d1f9fc47f95a2289950bae3c8a5438bcd924dcc67aa66`.
The separately bound native meter observes **−21.8 LUFS** integrated and
**−20.7 dBTP** true peak. These are measurements of a synthetic tone, not an
industry mastering target or musical-quality approval. Normalization is `none`;
native meter qualification remains false.

The stronger repeatable continuation passed in **3.680 seconds**. Its actual
observations include:

- Exact QC plan/request, retained `qc.json`, external receipt pin and read-only
  status agree. A wrong receipt pin produces the exact structured refusal.
- A clearly labeled synthetic 160×96/24-fps profile with matching dates and
  final-file QC returns `matches_supplied_profile`.
- Expired dates and omission of exact-final-file QC each return
  `missing_evidence` for their respective reason. A one-byte synthetic size
  ceiling returns `needs_changes`.
- The complete baseline inventory, including directories, both accepted videos,
  supplied lyrics and all original/selected executables, remains unchanged.
  Status and all preflight comparisons preserve the retained QC inventory.

The fixture's dates, numbers and checker text are explicitly synthetic. Its
official-domain URL is only a schema-required locator; it is not fetched and
does not establish current TikTok policy. A comparison match is not platform,
human, release or publication approval. The accepted PCM statistics retain their
typed unavailable loudness values; preflight uses the distinct bound native
meter observations instead of replacing unavailable with zero.

```sh
task publishing:qc-pilot \
  PILOT="/absolute/existing-passed-synthetic-pilot" \
  ANIFLOW="/absolute/explicit-current-aniflow" \
  OUTPUT="/absolute/fresh-qc-continuation"
```

Without Task, use `python3 -B scripts/publishing-qc-pilot.py --pilot-root ...
--aniflow ... --output ...`. It reuses exact tool pins from that passed pilot,
requires a disjoint fresh output root and retains private logs/failed receipts.
Input inventories are bounded to 4096 entries/512 MiB and executable identities
to 128 MiB. The reused watchdog monitors a 120-second CLI deadline and a 512-MiB
output budget; separate native process groups retain their existing bounds.
Descendant-tree cancellation remains unqualified.

Six focused Python observer cases pass, including unavailable/nonfinite values,
wrong subjects, changed masters/lyrics/empty directories and symbolic members.
The full Rust test suite, native-host matrix and actual human/publishing gates
remain separate. [The sanitized observation](FINAL-VIDEO-QC-OBSERVATION-20261114.json)
binds executable/harness/receipt hashes and actual comparisons without private
paths. Cropped/padded-source admission is unchanged; Flow #88 stays last.

## Later continuation

The [metadata/QC continuation](METADATA-FINISHING-QC-20261114.md) strengthens the
shared refusal observer to require native State exit 8, adds a panic-exit
counterexample and reruns this pilot on its fixed CLI. The original six-case
source/run observation above stays historical; the updated suite has seven
cases and the refreshed run has its own exact receipt.
