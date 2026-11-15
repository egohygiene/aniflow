# Ordinary MP4, supplied SRT and exact-final audio QC

This bounded synthetic path combines the two newly admitted source profiles: a 48-frame, 24 fps H.264 MP4 displaying 160×90 from a uniform 160×96 coded rectangle, with two-second 48 kHz stereo AAC presentation. All 96256 decoded audio sample frames, including 256 nonpresented frames with nonzero samples, are preserved. The source, lossless master and H.264 delivery must decode to the same complete PCM hash; AAC packet/configuration/clocks/priming must also remain identical.

The existing publishing, final-QC and metadata-QC pilots now accept an explicit `ordinary-mp4` profile. Their default aligned 64-frame profile retains its dimensions and requirements. No replacement media pipeline was added. The ordinary path binds the actual accepted extraction-evidence digest and exact coded/display rectangle. It converts a supplied Unicode SRT to WebVTT, preserves every word and interval, and requires native state exit 8 when unreviewed lyric sidecar export is refused. It does not perform caption mux/render qualification or approve words, timing, appearance or the final creative result.

With explicit local tools and a fresh existing parent for each output:

```sh
task publishing:pilot:ordinary ANIFLOW=/absolute/aniflow FFMPEG=/absolute/ffmpeg FFPROBE=/absolute/ffprobe OUTPUT=/scratch/source-pilot
task publishing:qc-pilot PILOT=/scratch/source-pilot ANIFLOW=/absolute/aniflow OUTPUT=/scratch/final-qc
task publishing:metadata-qc-pilot QC=/scratch/final-qc ANIFLOW=/absolute/aniflow OUTPUT=/scratch/metadata-qc
```

The same sequence can be invoked directly:

```sh
python3 scripts/publishing-pilot.py --source-profile ordinary-mp4 --aniflow /absolute/aniflow --ffmpeg /absolute/ffmpeg --ffprobe /absolute/ffprobe --output /scratch/source-pilot
python3 scripts/publishing-qc-pilot.py --pilot-root /scratch/source-pilot --aniflow /absolute/aniflow --output /scratch/final-qc
python3 scripts/publishing-metadata-qc-pilot.py --qc-pilot-root /scratch/final-qc --aniflow /absolute/aniflow --output /scratch/metadata-qc
```

Real FFmpeg and FFprobe 8.0.1 with the enumerated feature tables are required; the ordinary profile additionally checks `crop`. The final candidate is an optimized dev build from the actual published checkpoint-2 source head, with its own compiled contract and build observation. Cache reuse is explicit; this is not a formal release or independent reproducibility claim.

Final QC binds the exact final file and the complete decoded PCM, with scope `complete_decoded_pcm_including_nonpresented_tail`. Presented delivery loudness remains unverified. A synthetic dated structural profile can match when it does not require loudness; a profile requiring presented loudness remains missing evidence, and an explicit audio-policy comparison refuses the scope substitution. These supplied fixture requirements are not live platform specifications.

Metadata-only finishing creates a distinct file identity, preserves elementary packets and full decoded PCM, receives fresh matching QC, and refuses reuse of the previous file's QC. Compatible recipe/finishing resumes and all negative controls must preserve both original accepted videos and supplied lyrics.

Broader source profiles, presented-window metering, native supported-host matrix coverage, the known legacy alignment issue, descendant cancellation, actual Akashic assets, human review and publication remain separate work. No paid service, model or private asset is used.

## Observed checkpoint

The [portable checkpoint receipt](evidence/20261114/ordinary-publishing.json) binds the real published source `e73d35cfa167b0966f24df3d9662d26e2146de17`, clean source tree, optimized dev CLI, compiled contract and final harness files. The three main pilots passed in 16.710, 3.270 and 16.490 seconds, covering 33 actual product CLI calls and six exact native refusals. Complete PCM remained `b70f04f7cd3c64fbb6c80784e69f5d10ef29c5df3643695dfe04be47d0c2a4ee` across source, master, delivery, exact-final QC and metadata QC. The metadata file has a distinct identity and fresh matching QC. Thirteen new profile cases and seventeen existing observer cases passed; these observer tests launch no product/media process.

Full saved QC validation exposed a pre-existing composition omission: native-v3 accepted PCM correctly marks its unsupported loudness measurements unavailable, but the delivery summary accepted only v2 quantity shapes. The post-build schema-only repair changes exactly two delivery-summary refs to closed alternatives for the existing v2/v3 quantities. Native meter refs and global v2 schema remain unchanged. [Nineteen schema controls](evidence/20261114/qc-composition-schema.json) validate actual direct/metadata/resumable QC and decoded markers, legacy quantities, and refusal of unknown reasons, extra fields, wrong units and wrong numeric types. Neither repaired schema is a compiled descriptor subject; the captured executable and descriptor remain unchanged.

A separate focused resumable-QC observation completed run, status, completed resume and ordinary QC readback in 6.113 seconds using another already accepted native recipe owner. That owner's historical outer pilot had failed only a later observer exit-code expectation; its actual recipe/media outcome was native 0. The additional observation preserves the complete PCM, explicit tail scope and accepted marker. It uses native finite working bounds, process/capture limits and post-command output checks; it does not claim a live tree quota or interrupted-child cancellation qualification.

Failed observations remain retained: the first source harness expected configuration exit 2 instead of documented exit 4; the first QC harness looked for `media_kind` instead of the actual `video_geometry` check. An auxiliary live-tree monitor then raced a native atomic candidate-file rename and interrupted its resumable process; the next attempt correctly refused the retained stale writer lock. That lock remains on the private source02 assembly as failure evidence. All original video/SRT/accepted artifact bytes remain unchanged; the earlier whole-inventory observations describe their actual observation times. The successful independent resumable owner did not reuse or delete that stale lock.
