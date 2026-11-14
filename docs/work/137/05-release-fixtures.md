# Flow #137 checkpoint 5 — November 13, 2026

The release-plan immutability test passed a copied source checkout to the
production bundle inventory. The fixture includes `.egohygiene` and `.github`,
while distributable bundle inventory deliberately refuses leading-dot paths.
The current local baseline reproduced exactly that `unsupported artifact path`
error: 28 tests ran, with one error. The historical Actions report named 25
cases; the current source has grown and that older count is not reused.

The test now takes a separate bounded snapshot of its private source fixture.
Fixture construction uses an explicit repository-input allowlist. The snapshot
records the entire resulting private tree: directory/file membership, mode, byte count and SHA-256, including dot
paths, and refuses symlinks, special files and hard-link aliases. Individual
reads are capped at 2 MiB; the private tree is capped at 256 entries and 16 MiB.
Timestamps/atime are excluded so observing the fixture cannot invalidate itself.
This is a test oracle over a controlled temporary copy, not a filesystem snapshot
or a distributable archive validator.

Regression coverage checks hidden-byte edits, new directories/files and modes,
refuses an external symlink, and confirms that production bundle inventory still
rejects dot metadata. The actual invalid-version/date and dirty-prepare cases
compare the complete private fixture after every refused preparation attempt.
Production release/bundle helpers, archive naming, traversal/link defenses and
publication policy are unchanged.

## Focused local observation

On Python 3.12.14, the maintainer-authorized command:

```sh
python3 -B -m unittest discover --start-directory tests/release --pattern 'test_*.py'
```

passed **31 tests** with no skips after the repair and three new regressions.
The suite exercises synthetic temporary release fixtures and mocked remote/source
identity boundaries, including schema/refusal logic. It does not create a real
release, tag, attestation or workflow run. No Rust compilation/build/Clippy,
native media/model/hardware work, hosted CI or publication was performed.
The known alignment cancellation/start-marker cause remains unresolved, and
Flow #88 remains unchanged and last.

This checkpoint is stacked after the preceding Aniflow formatting draft. A
passing local fixture suite establishes this narrowly observed regression fix;
it does not qualify the accumulated Rust/source/native release matrix.
