# Prepare an explicit historical-frame selection

Use this #37 source checkpoint when a few preexisting PNGs need a current,
reviewable import request. Supply an original source/index, current decoder
settings, explicit provider locator, and 1..128 ordered original ordinals/files.
No directory scan, model guess, implicit reuse or import approval occurs.

The preparation request has schema
`aniflow.historical-import-preparation-request/v1`. Its `frames` entries contain
only the selected original ordinal and file locator:

```json
{"ordinal": 5, "file": "/private/selected frame.png"}
```

The other fields are `source_video`, `source_index`, `settings` and
`provider_executable`, matching the existing historical import request. All
locators are normalized absolute paths. `settings` is the existing ImportSettings
contract: current decoder, explicit uniform 2x/3x/4x output geometry and finite
storage bounds. Files and original ordinals must be distinct and ordered; an
explicit file-to-source relationship remains the operator's declaration.

```bash
aniflow --output json video historical-import prepare \
  --request "/private/selected files.json" \
  --output-request "/private/new import request.json"
```

Preparation observes the selected PNGs' current structure/CRC/geometry/bytes,
hashes and original frame times through existing owners, then reobserves the
exact review subject. It returns `aniflow.historical-import-preparation/v1`,
including an ordinary import request, its canonical SHA-256 and current import
review plan. The optional request file is private and no-clobber; omit the flag
for read-only JSON output. Existing files and all selected source/index/frames/
tools are protected. Preserve a partial request on an I/O failure and select a
new output path after inspection.

Review the actual selected files and separately supply the existing exact
ManualReview to `historical-import plan|run|resume`. Preparation does not decode
full RGB pixels, run tools, inspect a past environment or authenticate historic
models/settings. Existing native import performs current full decode and owns
acceptance. A later spatial carrier can consume only that actual accepted
import, never this preparation or loose PNGs. The provider locator is declared
but not pinned by preparation; ordinary native planning remains authoritative.

Four library and one CLI parser cases are authored, uncompiled/unrun. No checks,
native/media/provider/model/CLI/SDK execution, CI, real human review or source
replacement/disposal occurred. #37's remaining scope and qualification stay
open. Flow #88 remains last; its original ledger and the known unresolved
alignment cancellation/start-marker failure (cause unproven) are unchanged.
