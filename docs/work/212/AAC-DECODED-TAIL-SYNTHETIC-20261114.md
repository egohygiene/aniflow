# Complete AAC decode and presentation are distinct

Checkpoint 2 admits one explicitly observed 48 kHz AAC-LC profile: complete 1024-sample decoder blocks, one 1024-sample priming packet, contiguous packet/frame clocks, and a short final presented block without discard padding. Every nonfinal presented block must equal its native decoded count. The selected stream declaration must exactly match that presentation. Interior short blocks, gaps, extra packet side data, changed priming and unexplained declarations remain refused. This is not a general one-block tolerance.

The synthetic two-second MP4 presents 96000 samples but its 94 native AAC frames decode 96256 samples. Its final packet presents 768 samples from a 1024-sample block; the remaining 256 frames contain 512 nonzero finite channel values. The exact-main baseline refused the count mismatch. The fixed CLI completed the source recipe with native exit 0 and empty stderr, preserving the original full decoded PCM SHA256 `b70f04f7cd3c64fbb6c80784e69f5d10ef29c5df3643695dfe04be47d0c2a4ee` and AAC packet identities in the master and delivery.

No decoder trim, pad, resample or gain operation was added. `source_audio_extent` remains 0..2 seconds. Complete PCM/native-child/meter counts are 96256, with explicit decoded extent 0..752/375 seconds and metric scope `complete_decoded_pcm_including_nonpresented_tail`. These metrics do **not** verify a presented delivery loudness goal: policy assessment and saved policy validation refuse that substitution; required destination loudness remains missing evidence with an explicit reason.

The optional `decoded_audio_tail` extension is absent for legacy aligned inputs. Common temporal and AAC packet-identity serialization remain unchanged; the same padded source's temporal inspection was byte-equivalent before and after this change. Source signal and final/resumable QC use the same complete-count validator. Before final PCM is written, its observed complete size and additional working projection must fit the selected finite bounds; historical pre-probe intent bytes remain unchanged.

The [portable receipt](evidence/20261114/aac-decoded-tail.json) binds the actual private CLI/source observation and retained native/WAV audit. Twelve focused Rust source-module cases ran in a standalone wrapper, including real aligned/padded captures and native admission refusals. Twenty Python saved-capture controls and ten partial tail-schema controls passed; they launch no media process. The full product test suite was not run. One cfg(test) constructor was repaired after the private build; the final published-head checkpoint-3 build will include it. The build was optimized dev (Rust 1.85.0, opt-level 1, debug 0, incremental 0, two jobs), not a formal release.

To reobserve saved captures without decoding again:

```sh
python3 scripts/publishing-audio-tail-observer.py --probe AUDIO_PROBE.json --wav COMPLETE.wav --signal video-audio-signal.json
python3 tests/publishing_audio_tail/test_audio_tail_observer.py --capture-root CAPTURE_DIRECTORY
```

The observer binds the native declared/sample extents, exact full PCM, nonzero tail and receipt scope. Retained receipt readback rechecks actual PCM/native acceptance/meter identities; it does not claim to rerun native probes. Broader AAC profiles, a separately proved presented-window meter, native host qualification, legacy alignment, descendant cancellation, human reviews and publication remain separate work.
