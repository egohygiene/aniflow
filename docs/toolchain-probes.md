# Bounded toolchain probes

Checkpoint #101 adds an explicit probe operation to the first #38 setup draft;
#102 adds its macOS process adapter.
The implementation and synthetic fixtures are authored; all execution checks,
including native probes, remain unrun under #64.

Live process adapters are authored for Linux except uClibc targets and macOS.
Other hosts return an explicit configuration error. These target gates describe
implementation scope, not observed platform qualification. Offline doctor and
plan retain their existing host behavior.

`toolchain probe` asks explicitly pinned FFmpeg and ffprobe executables for
their diagnostic output. The library owns the argument lists. It accepts no
media input, shell command or arbitrary argument template. Existing
`toolchain doctor` and `toolchain plan` retain their offline inventory behavior.

```bash
aniflow --output json toolchain probe \
  --configuration "profiles/probes-core.example.json"
```

Replace the example's local paths and digest placeholders with reviewed files
before using it. Invoking this operation runs those selected executables. It
does not install tools, acquire models, register providers or start a media job.

## What the report can establish

| Query | Evidence | Limit |
| --- | --- | --- |
| Version | The selected tool's version banner and parsed release version | Vendor/nightly strings remain unavailable when they cannot be parsed without guessing. |
| Full help | Advertised option names | Advertising an option does not prove every invocation succeeds. |
| Encoders / decoders | Listed codec implementations | Presence does not qualify an encode or decode on actual media. |
| Filters | Listed filter implementations | Filter behavior still needs its own processing evidence. |
| Hardware acceleration methods | Backends compiled into the FFmpeg build | This does not establish an available GPU, driver, device or usable backend. |

FFprobe supplies version and full-help evidence. FFmpeg supplies all of the
above. Upscayl, gwr, allenk image/video and device-specific probes remain
separate #38 work; unsupported version flags are never invented for them.

Process outcome, bounded stdout/stderr, parse acceptance and executable identity
remain separate records. A successful exit alone does not produce accepted
capability evidence. Wrong-tool banners, malformed output, stale digests,
incomplete captures and failed commands keep the result unverified or failed.

Package revisions supplied by the caller remain attributed declarations;
version output does not authenticate the originating package or repository.
Native platform qualification remains false even when diagnostic collection
completes. A later native test must establish actual processing behavior.

`complete` describes probe collection and parsing. A missing caller package
revision does not change what the tool actually reported. The resulting
observation preserves that missing value, and offline doctor still reports
the missing package evidence if the observation is placed in an inventory.
Numeric `X.Y` releases are explicitly recorded as `X.Y.0` for comparison;
the original token and normalization method are retained. Vendor/nightly
tokens are preserved without guessing a compatible release.

Each accepted tool result exposes an `observation` compatible with the
inventory's observation field. Review it alongside the retained command
evidence before placing it under the matching artifact ID/path/digest. The
probe does not rewrite inventory files or fill hardware/model observations.
Observation flag/feature arrays allow up to 4,096 entries so full tool listings
fit; a profile's requested flag/feature lists remain limited to 128.

## Execution boundary

Only explicitly named, locally pinned executables are considered. Arguments
are passed directly to `Command`; paths containing spaces or shell characters
remain data. A private temporary working directory and a minimal deterministic
environment limit ambient configuration. Per-command and aggregate deadlines,
bounded stdout/stderr capture and cancellation apply to the probe session.

The example permits five seconds per command and 60 seconds for the session,
one MiB per output stream, eight MiB of total retained capture, a 512 MiB
executable and two GiB of total executable hashing. Every query checks the
selected executable before and after it runs, charging each read to that budget.
Large static builds may need a deliberately larger hash budget. Cleanup after a
deadline or cancellation allows the configured termination grace, followed by
a reap interval of the greater of that grace or 100 milliseconds, plus polling
and scheduling overhead. The leader stays unreaped until all process-group
signals have been sent, so cleanup cannot target a reused process-group ID.
Configuration validation enforces fixed upper bounds; these are not benchmark
or native performance claims.

Linux uses non-reaping `waitid` observations. macOS uses a private `kqueue`
process-exit subscription. If an already-exiting child cannot be attached,
the adapter retains that separate state and continues bounded capture until
natural pipe closure or a stop condition. It completes group signaling before
disarming signals and reaping; only the real child wait status can establish
process success. Closing stdout
and stderr alone never establishes exit. Observation errors retain failure
evidence rather than producing an accepted capability result.

The embedding application must leave this operation sole ownership of waiting
for its direct children. It must not reap them elsewhere or configure automatic
child reaping through `SIGCHLD` ignore/`SA_NOCLDWAIT`. This ownership requirement
preserves the process-ID lifetime used by cleanup. Process groups do not confine
descendants that deliberately create a new session or otherwise escape the
group; the trusted-executable boundary still applies.

The original executable path is used. Before/after hashes detect observed
changes; they are not atomic attestation of the exact file executed and do not
defend against a hostile process replacing and restoring it between checks.
Retained stdout/stderr bytes are hex-encoded with their own digest. Observed
byte counts exclude bytes left unread after termination, and incomplete
captures stay marked truncated. Human output previews are bounded; JSON
retains the full captured evidence within the configured limits.

Selected tools run with the user's authority. Fixed informational arguments
and environment control are not an operating-system sandbox, a network denial
mechanism or proof that an untrusted executable has no side effects. The caller
must review and trust the pinned programs. Aniflow itself performs no network
request or dependency installation in this operation.

Probe evidence does not select a runtime provider or establish a provider lock.
Keep the original configuration and resulting report for review. Package,
backend, model and adapter identity must still be bound through the existing
explicit setup and Pipeline v3 registration path before processing.

See [toolchain profiles](toolchain-profiles.md), [upstream evidence](toolchain-upstreams.md)
and [deferred qualification #64](https://github.com/egohygiene/aniflow/issues/64).

## Authored host coverage

The synthetic process fixture in `tests/toolchain_probes.rs` targets both live
host adapters. It requires a caller-prepared Python 3.10+ interpreter only for
tests; normal FFmpeg/ffprobe probing has no Python dependency. The fixture uses
the absolute `ANIFLOW_TEST_PYTHON` path when supplied, otherwise `/usr/bin/python3`.
It resolves that path to an executable regular file and requires no whitespace
or control characters and a shebang line of at most 127 bytes. Missing or invalid
prerequisites fail with an actionable message; cases are not silently skipped
and no interpreter is installed or found by searching `PATH`.

macOS source inspection informed this implementation, but no macOS compiler,
synthetic fixture or real FFmpeg/ffprobe execution has occurred here. Linux
regression execution is also deferred. Host architecture, interpreter, selected
binary and runtime results must be recorded during later #64 qualification.
