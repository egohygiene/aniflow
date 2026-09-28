# Musical estimates checkpoint (#46)

Parent mini roadmap: [#13](https://github.com/egohygiene/aniflow/issues/13).
This is checkpoint 5/10. PR #57 merged #45 before this work; fresh main is
`6c767c55d06929efc1de49c16bebce391415522a`.

## Checkpoint 1: analyzer selection before integration

Live issues, PRs, releases, repository instructions and the current Flow #11
handoff were refreshed. #42 and #43 are merged prerequisites; #46 is ready.
The implementation remains one review PR and does not close parent #13.

Select the optional Essentia Python package `2.1b6.dev1389` with NumPy `2.3.5`.
An isolated development environment successfully installed those exact software
packages, plus PyYAML `6.0.3` and six `1.17.0`. No model weights were downloaded.
The runtime reports Essentia `2.1-beta6-dev`, source revision
`v2.1_beta5-1389-g36ec3d92`, on Python `3.12.14`. This is a prerelease package;
record its full identity rather than treating its short runtime version as an
exact build pin.

The narrow adapter will call `RhythmExtractor2013(method="multifeature")` and
`KeyExtractor` with separately retained `krumhansl` and `temperley` profiles.
It will preserve competing BPM observations and key profiles without choosing a
consensus. Raw agreement/correlation scores are not calibrated probabilities;
public confidence remains explicitly unavailable.

The initial profile accepts 44.1 kHz PCM16 mono or stereo, with explicit
arithmetic channel averaging, at least eight seconds and at most ten minutes.
It does not resample, normalize, apply replay gain, or infer individual-channel
estimates from a mixed signal. Silence, insufficient duration and other sample
rates produce unavailable estimates. Wider musical feature families remain
explicitly unsupported. Source bytes and existing stem lineage remain intact.

Essentia offers the required tempo, beat and tonal-key algorithms in one existing
offline library. Librosa supplies beat/chroma operations but would require an
additional key estimator; aubio supplies tempo/pitch but no corresponding tonal
key estimator. The broad Essentia MusicExtractor executable is unnecessary and
has preprocessing/fallback behavior outside this bounded profile. A small
replaceable Python adapter makes its exact preprocessing and algorithms visible.

Primary references:

- [RhythmExtractor2013](https://essentia.upf.edu/reference/std_RhythmExtractor2013.html)
- [KeyExtractor](https://essentia.upf.edu/reference/std_KeyExtractor.html)
- [Key profiles](https://essentia.upf.edu/reference/std_Key.html)
- [Package 2.1b6.dev1389](https://pypi.org/project/essentia/2.1b6.dev1389/)
- [Upstream licensing](https://essentia.upf.edu/licensing_information.html)

The installed Essentia wheel metadata declares `AGPL-3.0-only`. Its CPython 3.12
Linux x86-64 wheel SHA-256 is
`5905c38da25d8ccf4259bd045e50242103a47f482b77559ec6899d4b463ca4e7`;
installed `COPYING.txt` SHA-256 is
`857d4e8afe59718161905db0295c7e09d00674e0be844c2a0500465afbe06521`.
NumPy's installed license file, including bundled-library notices, hashes to
`2046a3130e50b11c01659b3a0d963e6ae0b7436ff8e89cbcfd9e87bc6112d595`.
The provider will record exact installed distribution identities and license
evidence. No runtime, weights or commercial licensing clearance is bundled or
claimed by this optional adapter. These two algorithms require no model weights.

Implementation, schemas, fixtures and validation remain in progress. Planned
checks distinguish deterministic adapter conformance from limited synthetic
analyzer observations. No general musical accuracy or release qualification is
claimed. Hosted CI is not a blocking or repeatedly polled gate. The maintainer
reviews and merges the finished PR.
