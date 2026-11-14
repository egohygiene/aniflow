# Quest and DeoVR playback references (#229)

Disposition: retain the three helpers as optional playback references, with no
new export engine or product target in this intake. Their exact originals and
hashes are in the [source index](README.md). This is a clear disposition of
[#229](https://github.com/egohygiene/aniflow/issues/229), not a device-compatibility
receipt or an expansion of the current destination contract.

## What each source contributes

| Source behavior | Current owner and decision |
| --- | --- |
| `normalize-video` encodes first actual video to H.264 High, level5.1 by default, CRF17, yuv420p, even padded dimensions and faststart. Level, CRF and preset are explicit options. | [Destination video](../destination-video.md), [`destination::video`](../../src/destination/video/mod.rs) and [`video_assembly::media::destination`](../../src/video_assembly/media/destination.rs) own explicit geometry/encoding and exact source-clock validation. Keep this as a trial configuration reference; do not copy its H.264 level or CRF as a universal device rule. |
| It retains all AAC tracks, converts other audio to AAC unless explicit copy is selected, refuses subtitles and recognized HDR transfer functions, and carries inherited metadata/chapters. | Existing [assembly](../video-assembly.md), [metadata policy](../delivery-metadata.md), [subtitle delivery](../caption-delivery.md) and selected media admission own these choices. The current closed single-selected-AAC profile does not cover every trial audio layout. Keep HDR/subtitle refusal lessons; no implicit audio conversion, track loss or metadata policy is adopted. |
| It probes only encoded codec/pixel format before final installation, supports `--force`, and can explicitly overwrite an MP4 source. | Keep the useful temporary-file/validation pattern, but reject source overwrite and replacement of prior exports for Aniflow integration. Existing native stages preserve source, create isolated candidates and require their complete independent media validation. A successful codec probe alone is insufficient. |
| `to-vr` explicitly assumes full-range BT.709 input, converts `pc` to `tv`, sets square pixels and BT.709 tags, copies audio and writes a new faststart H.264 MP4. | [`GeometryRule::filter`](../../src/destination/video/intent.rs) already builds an observed-range-aware filter inside its supported profile. Retain deliberate range conversion as a lesson; never apply this helper's unprobed full-range assumption or replace color/SAR tags to conceal incompatible content. |
| `check_deovr.sh` reads first video/audio metadata and prints heuristics for codecs, H.26410-bit, width above8192 and audio codecs, plus a suggested VideoToolbox command. | [Destination preflight](../destination-preflight.md) compares accepted observations with explicit dated requirements and reports missing evidence. Keep the fields as questions for a future profile. The script is a human hint, not a machine acceptance check or a hardware limit authority. |

## Why an existing target is not relabeled

The current [`Destination`](../../src/destination/types.rs) enum contains
TikTok, SoundCloud and DistroKid. Video export configuration explicitly selects
TikTok in [`VideoExportConfiguration::validate`](../../src/destination/video/intent.rs).
There is no Quest/DeoVR profile to configure today. Passing a playback helper's
settings through an unrelated existing destination would misstate what was
checked. Generic H.264 output is not proof of device or application support.

The checker does not inspect codec level, bitrate, frame rate, container
constraints, projection/stereo metadata, device/player version or every audio
track. It does not stop reliably on an FFprobe failure, and later failures
replace earlier reason text. Its “friendly” result therefore cannot qualify
even all properties it prints. Do not promote its hard-coded8K width statement,
10-bit crash description or suggested hardware encoder into supported policy.

None of these scripts creates or validates VR180/360 projection, stereo layout
or player mode. Explicit `setsar=1` can change intended display geometry if its
input assumption is wrong. Range conversion, tone mapping, audio conversion
and geometry changes require separately selected semantics, not filename-driven
presets. Retain the accepted clean master and exact source/frame/audio clocks.

## A bounded future profile, if selected under #212

Add one explicit local playback destination through the existing destination
owner, with a versioned closed request and dated operator-supplied requirements:

- identify the exact device, player/version, container, codec/profile/level,
  frame rate, bitrate, geometry, color/range and selected audio requirements;
- declare whether projection/stereo metadata is applicable and how it is
  preserved or validated; missing evidence stays missing;
- reuse current source/accepted-output identity, geometry, metadata and temporal
  validators; refuse unsupported HDR, subtitles or tracks instead of dropping
  them, and preserve a separate original master;
- add synthetic refusal/argument cases, then perform a separately recorded
  intended-device playback check, including motion, audio sync and player mode.

That would be new supported-profile work, not a small safe default change.
This intake adds no target, range conversion, native encoder or compatibility
claim. Only source/hash/link inspection was performed; no media or device ran.
