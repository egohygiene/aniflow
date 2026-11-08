# Source-bound video audio signal checkpoint

November 7, 2026. Base: main `0242a0554522a7fc4fe72651b13a6b772d425331`.
#35 stays open; #64 retains all runtime/release qualification and the known
unresolved alignment cancellation/start-marker failure.

The public `delivery_audio::source_signal::measure` composes exact selected-video
audio decoding with the existing #13 native PCM analysis and meter owners.
Inputs name the exact original video SHA-256/size, explicit video/audio/discard
selection, existing assembly tool/resource settings, native provider executable
and maximum PCM bytes. The output must be a fresh absolute private directory.
The [Rust example](../../examples/video_audio_signal.rs) accepts an explicit
request document; it is authored and must not be read as an executed command.

The request schema token is `aniflow.video-audio-signal-request/v1`. Assembly
configuration remains `aniflow.video-assembly-configuration/v1`. Source and all
three executables are copied with exact byte pins. Existing closed MP4/MOV
source/AAC-LC stream/packet/color/timing observation is reused. The first meter
subset is 48 kHz mono or stereo. Other rates/layouts are refused; no resampling,
downmix, normalization, trimming, padding or original-mix rewrite is introduced.

Float32 decoding preserves the native analysis representation and finite overs.
The complete decoded audio timeline must be contiguous, on its exact sample
grid; the WAV sample count/rate/channels/format must match it. Audio origin and
extent remain separate from the WAV's zero sample origin. FFmpeg's approximate
`-fs` is a protective file-size request, not an OS quota or completeness proof;
post-read byte limits and complete expected frame count refuse any truncated
output. No delivery/master is reencoded. Snapshots, tool captures, decoded WAV
and native child runs can remain after failure, with no completion receipt.

The existing v3 technical/native-signal child accepts the exact PCM artifact.
Its native-v3 meter fields intentionally remain unavailable. A separately
versioned `aniflow.native-pcm-loudness/v1` receipt runs the existing #13 two-pass
EBU meter commands/parsers directly on float32, checks complete meter windows
and independently observed native sample peaks, and retains typed silence,
duration and absolute-gate unavailability. Its measured values are new scoped
observations with `runtime_qualified: false`, not a reinterpretation or mutation
of accepted native-v3 JSON or a loudness-normalization target.

The final private `aniflow.video-audio-signal/v1` receipt binds original source,
selected stream, rational extent, decoded WAV/PCM identity, tools/provider,
accepted signal child plan/report and meter subject. Before writing it, source,
PCM, tool identity and child acceptance are rechecked. Existing v3 acceptance
remains authoritative; this composition receipt grants no new artifact authority.
Free space/working estimates and tool cancellation are cooperative, without a
hard disk/RSS/VRAM quota or a preemptible whole-operation deadline. The copied
tools remain trusted native code; no OS sandbox claim is added.

The 48 kHz mono/stereo float32 WAV choice was checked against FFmpeg n8.0.1
`libavformat/riffenc.c` and `wavenc.c`: this subset does not need extensible WAV,
and it uses the existing bounded RIFF32/float-fact reader. These are source
references, not native execution evidence.

Four focused regressions are authored, uncompiled/unrun: exact decode argv,
source-stream/complete-time binding with gap refusal, typed short/silent values,
and finite overs/true-peak consistency. No tests/compiler/build/Cargo,
lint/format/schema/docs checks, native/provider/media/model/CLI execution,
dependency installation or CI ran. No Cargo dependencies/lockfiles, old signal
schemas, golden fixtures, source audio, releases or issue acceptance boxes changed.
Broader formats/rates, richer workflow/CLI composition and native/platform/audio
quality acceptance remain in #35/#64; no extra issue is created.
