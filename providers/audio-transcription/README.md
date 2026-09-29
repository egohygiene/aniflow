# Native observed transcription provider

This bundle declares the first-party, bounded whisper.cpp transcription stage.
Use the public `aniflow::audio_transcription` facade or `aniflow audio transcribe`
to compose technical inspection, optional verified stem lineage, and this stage
through Pipeline v3. The provider executable uses the fixed
`--aniflow-invocation <absolute-request-path>` ABI.

The initial profile is whisper.cpp 1.8.7, a caller-supplied pinned `tiny.en` model,
explicit English, and PCM16 WAV at 16000 Hz with one channel. It produces observed
segment text and source-relative segment times. It does not establish reviewed
lyric authority, word timing, calibrated confidence, translation, VAD, diarization,
or lyric alignment. Unsupported rates/channels and exactly silent mono PCM have
explicit unavailable outcomes. Empty analyzer output retains an empty transcript
result without inventing an empty timed-text cue.

The two fixed outputs are `transcription.json` and normalized `analysis.json`.
The transcription companion embeds a normalized timed-text document only for a
nonempty observed transcript. Raw basic-JSON stdout is bounded and independently
parsed; its digest and size are retained, but its private model path is not
published as an artifact or preserved in normalized evidence.

## Explicit local dependencies

The configuration pins the executable path, SHA-256 and exact version, plus the
model path, SHA-256, byte count, `tiny.en` identity and revision. These are caller
claims checked against local bytes; the adapter does not authenticate an upstream
model origin. The provider lock includes the exact tool/model digests. The model
component version `1.0.0` names this integration profile, while the model's own
revision remains explicit in configuration and evidence.

No model or executable is downloaded. A missing, changed or unsupported dependency
fails preflight with a typed diagnostic. Planning and resume reobserve dependencies
before resolving or reusing any checkpoint. Read-only Pipeline v3 status does not
run the transcription tool or establish current dependency readiness.

For each probe or execution the pinned executable is copied into a private empty
directory. During inference, the model and source WAV are copied there too, with
fixed private names and digest checks. Original adjacent CoreML/OpenVINO assets or
GGML backend files are not copied. The runner clears inherited environment values,
uses a fixed system PATH, directs temporary files to the private directory, and
invokes direct arguments without a shell. The CPU arguments disable GPU and flash
attention, use one processor, and bound threads; basic JSON is captured from
stdout rather than written to an undeclared output file. The original and staged
bytes are rechecked before publishing evidence, and private staging is removed.

The executable remains explicitly trusted native code. Its dynamic system
libraries and compiled-in backend directories are not covered by its byte hash;
this process boundary is not an operating-system sandbox or a complete dependency
closure. A staged executable that requires adjacent shared libraries is unavailable
under this profile. Local synthetic executable tests establish protocol behavior,
not transcription accuracy or real-model qualification.

## Bounds and output authority

Tool stdout and stderr are independently bounded to the configured 1 KiB–1 MiB
limit, with a child deadline of at most 120 seconds. The executable is at most
128 MiB; the model is at most 256 MiB; technical inspection limits source WAV to
256 MiB and 600 seconds. Exact artifact paths are generated beneath the existing
isolated Pipeline v3 workspace. All media inputs remain immutable.

`TranscriptionRequest::new` retains `inspection.execution_limits`. Callers working
with larger inputs/models may explicitly use
`audio_transcription::default_execution_limits()` (704 MiB artifact-tree bound)
when constructing the inspection request. This allows the transient private copies
within their independent size limits. It does not enlarge the configured child
capture or timeout limits. Completed stages still declare only two output files.
