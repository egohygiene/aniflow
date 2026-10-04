//! Bounded fixed-command probes of explicitly pinned trusted local executables.
//! Before/after hashes are observations, not atomic executed-file attestation.
//! Environment isolation and process groups are not an operating-system sandbox.
use super::probe_types::*;
use crate::{CancellationToken, Result};
#[cfg(not(any(target_os = "macos", all(target_os = "linux", not(target_env = "uclibc")))))]
use crate::{Error, ErrorCategory};
#[cfg(target_os = "macos")]
#[path = "probe_macos.rs"]
mod macos;

/// Run only the fixed diagnostic queries for explicitly pinned executables.
///
/// This checkpoint supports macOS and Linux hosts other than uClibc; other hosts
/// return a configuration error. The caller must retain exclusive ownership of
/// child waits and must not enable SIGCHLD auto-reaping. A complete report describes parsed diagnostic
/// evidence, not native media qualification or hardware availability.
pub fn probe_tools(
    configuration: &ToolchainProbeConfiguration,
    cancellation: &CancellationToken,
) -> Result<ToolchainProbeReport> {
    configuration.validate()?;
    #[cfg(any(target_os = "macos", all(target_os = "linux", not(target_env = "uclibc"))))]
    { unix::probe(configuration, cancellation) }
    #[cfg(not(any(target_os = "macos", all(target_os = "linux", not(target_env = "uclibc")))))]
    {
        let _ = cancellation;
        Err(Error::new(ErrorCategory::Configuration,
            "bounded toolchain process probes require macOS or Linux with non-reaping waitid support"))
    }
}

#[cfg(any(target_os = "macos", all(target_os = "linux", not(target_env = "uclibc"))))]
mod unix {
    use std::fs::{self, OpenOptions};
    use std::io::{self, Read};
    use std::os::fd::AsFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    use nix::fcntl::{FcntlArg, OFlag, fcntl};
    use nix::sys::signal::{Signal, killpg};
    #[cfg(target_os = "linux")]
    use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};
    use nix::unistd::Pid;
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::toolchain::probe_parse::{parse_probe_observation, probe_commands};
    use crate::toolchain::ToolchainStatus;

    struct PinFailure { status: ToolchainStatus, observed: Option<String>, detail: String }
    fn pin_failure(status: ToolchainStatus, detail: impl Into<String>) -> PinFailure {
        PinFailure { status, observed: None, detail: detail.into() }
    }
    fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
        b.is_file() && a.dev() == b.dev() && a.ino() == b.ino() && a.len() == b.len()
            && matches!((a.modified(), b.modified()), (Ok(a), Ok(b)) if a == b)
    }
    fn interrupted(cancellation: &CancellationToken, deadline: Instant) -> Option<&'static str> {
        if cancellation.is_cancelled() { Some("probe cancelled") }
        else if Instant::now() >= deadline { Some("total probe deadline exhausted") }
        else { None }
    }

    fn hash_pin(tool: &ToolchainProbeTool, limits: &ToolchainProbeLimits,
        remaining: &mut u64, deadline: Instant, cancellation: &CancellationToken,
    ) -> std::result::Result<String, PinFailure> {
        if let Some(detail) = interrupted(cancellation, deadline) {
            return Err(pin_failure(ToolchainStatus::Unverified, detail));
        }
        let before = fs::symlink_metadata(&tool.path).map_err(|error| pin_failure(
            if error.kind() == io::ErrorKind::NotFound { ToolchainStatus::Missing } else { ToolchainStatus::Unverified },
            format!("cannot inspect {}: {error}", tool.path.display())))?;
        if !before.is_file() || before.len() == 0 || before.len() > limits.maximum_executable_bytes
            || before.permissions().mode() & 0o111 == 0 {
            return Err(pin_failure(ToolchainStatus::Incompatible,
                "probe target must be a bounded nonempty executable regular file, not a symlink"));
        }
        if before.len() > *remaining {
            return Err(pin_failure(ToolchainStatus::Unverified, "total executable hash budget exhausted"));
        }
        *remaining -= before.len();
        let result = (|| -> io::Result<String> {
            let file = OpenOptions::new().read(true)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK).open(&tool.path)?;
            if !same_file(&before, &file.metadata()?) { return Err(io::Error::other("target changed during open")); }
            let mut input = file.take(before.len() + 1);
            let mut digest = Sha256::new();
            let mut bytes = [0_u8; 65536];
            let mut total = 0_u64;
            loop {
                if let Some(detail) = interrupted(cancellation, deadline) { return Err(io::Error::other(detail)); }
                let count = input.read(&mut bytes)?;
                if count == 0 { break; }
                total += count as u64;
                if total > before.len() { return Err(io::Error::other("target grew during hashing")); }
                digest.update(&bytes[..count]);
            }
            if total != before.len() || !same_file(&before, &input.get_ref().metadata()?)
                || !same_file(&before, &fs::symlink_metadata(&tool.path)?) {
                return Err(io::Error::other("target changed during hashing"));
            }
            Ok(format!("{:x}", digest.finalize()))
        })();
        let digest = result.map_err(|error| pin_failure(ToolchainStatus::Unverified,
            format!("could not establish stable identity for {}: {error}", tool.path.display())))?;
        if digest != tool.expected_sha256 {
            return Err(PinFailure { status: ToolchainStatus::Incompatible, observed: Some(digest.clone()),
                detail: format!("{} SHA-256 {digest} differs from supplied pin {}", tool.path.display(), tool.expected_sha256) });
        }
        Ok(digest)
    }

    // The unreaped leader reserves its PID while capture pipes are open. Never
    // send a group signal after reaping: that numeric PGID could be reused.
    struct ProcessGroup {
        child: Child,
        may_signal: bool,
        #[cfg(target_os = "macos")]
        exit_observer: Option<macos::ExitObserver>,
    }
    impl ProcessGroup {
        #[cfg(target_os = "linux")]
        fn observe_exit(&mut self, _captures_closed: bool) -> io::Result<Option<ExitStatus>> {
            match waitid(Id::Pid(Pid::from_raw(self.child.id() as i32)),
                WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT) {
                Ok(WaitStatus::Exited(_, code)) => Ok(Some(ExitStatus::from_raw(code << 8))),
                Ok(WaitStatus::Signaled(_, signal, core)) =>
                    Ok(Some(ExitStatus::from_raw(signal as i32 | if core { 0x80 } else { 0 }))),
                Ok(_) => Ok(None),
                Err(error) => {
                    // Another owner reaping our child invalidates the PID
                    // lifetime guarantee. Refuse to signal a possibly reused ID.
                    if error == nix::errno::Errno::ECHILD { self.may_signal = false; }
                    Err(io::Error::from(error))
                }
            }
        }
        #[cfg(target_os = "macos")]
        fn observe_exit(&mut self, captures_closed: bool) -> io::Result<Option<ExitStatus>> {
            if matches!(self.exit_observer, Some(macos::ExitObserver::ExitingBeforeRegistration)) {
                // ESRCH proves no status. Preserve the PID and drain naturally
                // first: killing pipe-owning descendants here would fabricate
                // complete capture. Open inherited pipes must hit the deadline.
                if !captures_closed { return Ok(None); }
                // With both ESRCH registration and natural EOF, send the final
                // group signal while the unreaped child reserves its PID, then
                // disable signals before obtaining the actual wait status.
                self.kill_before_reap();
                return self.child.try_wait();
            }
            self.exit_observer.as_mut()
                .ok_or_else(|| io::Error::other("macOS child exit observer was not registered"))?.poll()
        }
        fn kill_before_reap(&mut self) {
            if self.may_signal {
                signal(self, Signal::SIGKILL);
                let _ = self.child.kill();
                self.may_signal = false;
            }
        }
    }
    impl Drop for ProcessGroup {
        fn drop(&mut self) {
            self.kill_before_reap();
            let _ = self.child.try_wait();
        }
    }
    fn signal(group: &ProcessGroup, value: Signal) {
        if group.may_signal { let _ = killpg(Pid::from_raw(group.child.id() as i32), value); }
    }
    fn reap(group: &mut ProcessGroup, grace: Duration, terminate: bool) -> Option<ExitStatus> {
        if terminate {
            signal(group, Signal::SIGTERM);
            let grace_deadline = Instant::now() + grace;
            while Instant::now() < grace_deadline {
                // Observe without reaping so every later group signal is still
                // protected by the reserved leader PID, even after leader exit.
                if group.observe_exit(false).is_err() { break; }
                thread::sleep(Duration::from_millis(5));
            }
        }
        group.kill_before_reap();
        let mut status = None;
        let reap_deadline = Instant::now() + grace.max(Duration::from_millis(100));
        while status.is_none() && Instant::now() < reap_deadline {
            if let Ok(Some(exit)) = group.child.try_wait() { status = Some(exit); }
            if status.is_none() { thread::sleep(Duration::from_millis(5)); }
        }
        status
    }
    fn nonblocking(pipe: &impl AsFd) -> io::Result<()> {
        let flags = fcntl(pipe, FcntlArg::F_GETFL).map_err(io::Error::from)?;
        fcntl(pipe, FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK))
            .map_err(io::Error::from)?;
        Ok(())
    }
    #[derive(Default)]
    struct Capture { bytes: Vec<u8>, total: u64, eof: bool, truncated: bool }
    fn bounded_detail(value: String) -> String {
        let mut printable = String::with_capacity(value.len().min(8192));
        for character in value.chars() {
            if printable.len() + character.len_utf8() > 8176 {
                printable.push_str(" [truncated]");
                break;
            }
            printable.push(if character.is_control() { ' ' } else { character });
        }
        printable
    }
    fn poll_capture(input: &mut impl Read, capture: &mut Capture, maximum: u64,
        total: &mut u64, total_maximum: u64,
    ) -> io::Result<bool> {
        if capture.eof { return Ok(false); }
        let allowed = maximum.saturating_sub(capture.bytes.len() as u64)
            .min(total_maximum.saturating_sub(*total));
        let amount = usize::try_from((allowed + 1).min(8192)).expect("bounded read");
        let mut buffer = [0_u8; 8192];
        match input.read(&mut buffer[..amount]) {
            Ok(0) => { capture.eof = true; Ok(false) }
            Ok(count) => {
                capture.total += count as u64;
                *total += count as u64;
                let retained = count.min(usize::try_from(allowed).unwrap_or(usize::MAX));
                capture.bytes.extend_from_slice(&buffer[..retained]);
                capture.truncated |= retained != count;
                Ok(capture.total > maximum || *total > total_maximum)
            }
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => Ok(false),
            Err(error) => Err(error),
        }
    }
    fn command_result(id: &str, arguments: &[String], outcome: ToolchainProbeCommandOutcome,
        status: Option<ExitStatus>, stdout: Capture, stderr: Capture, started: Instant, detail: Option<String>,
    ) -> ToolchainProbeCommandResult {
        ToolchainProbeCommandResult {
            command_id: id.to_owned(), arguments: arguments.to_vec(), outcome,
            exit_code: status.as_ref().and_then(ExitStatus::code), signal: status.as_ref().and_then(ExitStatusExt::signal),
            stdout: ToolchainProbeCapture::from_bytes(&stdout.bytes, stdout.total, stdout.truncated),
            stderr: ToolchainProbeCapture::from_bytes(&stderr.bytes, stderr.total, stderr.truncated),
            duration_milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            detail: detail.map(bounded_detail),
        }
    }
    fn run_command(tool: &ToolchainProbeTool, id: &str, arguments: &[String],
        limits: &ToolchainProbeLimits, deadline: Instant, cancellation: &CancellationToken,
        total_capture: &mut u64,
    ) -> ToolchainProbeCommandResult {
        use ToolchainProbeCommandOutcome as Outcome;
        let started = Instant::now();
        let directory = match tempfile::Builder::new().prefix("aniflow-tool-probe-").tempdir() {
            Ok(value) => value,
            Err(error) => return command_result(id, arguments, Outcome::SpawnFailed, None,
                Capture::default(), Capture::default(), started, Some(format!("cannot create private probe directory: {error}"))),
        };
        let mut command = Command::new(&tool.path);
        command.args(arguments).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .env_clear().env("LC_ALL", "C").env("LANG", "C").env("PATH", "/usr/bin:/bin")
            .env("HOME", directory.path()).env("TMPDIR", directory.path()).current_dir(directory.path()).process_group(0);
        if let Some(detail) = interrupted(cancellation, deadline) {
            return command_result(id, arguments, if cancellation.is_cancelled() { Outcome::Cancelled } else { Outcome::TimedOut },
                None, Capture::default(), Capture::default(), started, Some(detail.to_owned()));
        }
        let mut group = match command.spawn() {
            Ok(child) => ProcessGroup { child, may_signal: true,
                #[cfg(target_os = "macos")]
                exit_observer: None,
            },
            Err(error) => return command_result(id, arguments, Outcome::SpawnFailed, None,
                Capture::default(), Capture::default(), started, Some(format!("probe spawn failed: {error}"))),
        };
        #[cfg(target_os = "macos")]
        let observation_setup = macos::ExitObserver::register(group.child.id())
            .map(|observer| group.exit_observer = Some(observer));
        #[cfg(target_os = "linux")]
        let observation_setup: io::Result<()> = Ok(());
        let mut output = group.child.stdout.take().expect("piped stdout");
        let mut errors = group.child.stderr.take().expect("piped stderr");
        let grace = Duration::from_millis(limits.termination_grace_milliseconds);
        if let Err(error) = observation_setup.and_then(|()| nonblocking(&output)).and_then(|()| nonblocking(&errors)) {
            drop(output); drop(errors);
            let status = reap(&mut group, grace, true);
            let reaped = status.is_some();
            return command_result(id, arguments,
                if reaped { Outcome::CaptureFailed } else { Outcome::CleanupFailed }, status,
                Capture { truncated: true, ..Capture::default() },
                Capture { truncated: true, ..Capture::default() }, started,
                Some(format!("cannot configure bounded process observation: {error}; leader reaped: {reaped}")));
        }
        let command_deadline = deadline.min(started + Duration::from_millis(limits.timeout_milliseconds));
        let (mut stdout, mut stderr) = (Capture::default(), Capture::default());
        let mut status = None;
        let (mut outcome, mut detail) = loop {
            if cancellation.is_cancelled() { break (Outcome::Cancelled, Some("probe cancelled".to_owned())); }
            if Instant::now() >= command_deadline { break (Outcome::TimedOut, Some("probe deadline exceeded before exit and complete pipe closure".to_owned())); }
            let stdout_poll = poll_capture(&mut output, &mut stdout, limits.maximum_stdout_bytes,
                total_capture, limits.maximum_total_capture_bytes);
            let stderr_poll = if stdout_poll.as_ref().is_ok_and(|limited| !*limited) {
                poll_capture(&mut errors, &mut stderr, limits.maximum_stderr_bytes, total_capture, limits.maximum_total_capture_bytes)
            } else { Ok(false) };
            match (stdout_poll, stderr_poll) {
                (Err(error), _) | (_, Err(error)) => break (Outcome::CaptureFailed, Some(format!("probe capture failed: {error}"))),
                (Ok(true), _) | (_, Ok(true)) => break (Outcome::OutputLimit, Some("probe exceeded a stream or total capture bound".to_owned())),
                _ => {}
            }
            match group.observe_exit(stdout.eof && stderr.eof) {
                Ok(value) => { if value.is_some() { status = value; } }
                Err(error) => break (Outcome::CaptureFailed, Some(format!("probe process status failed: {error}"))),
            }
            if let Some(exit) = &status {
                if stdout.eof && stderr.eof {
                    break (if exit.success() { Outcome::Succeeded } else { Outcome::Failed }, None);
                }
            }
            thread::sleep(Duration::from_millis(5));
        };
        stdout.truncated |= !stdout.eof;
        stderr.truncated |= !stderr.eof;
        drop(output); drop(errors);
        let reaped = reap(&mut group, grace, !(status.is_some() && stdout.eof && stderr.eof));
        if reaped.is_none() {
            outcome = Outcome::CleanupFailed;
            detail = Some("SIGKILL was requested but the leader could not be reaped within the bounded cleanup interval".to_owned());
        }
        status = reaped.or(status);
        command_result(id, arguments, outcome, status, stdout, stderr, started, detail)
    }

    pub(super) fn probe(configuration: &ToolchainProbeConfiguration, cancellation: &CancellationToken) -> Result<ToolchainProbeReport> {
        let deadline = Instant::now() + Duration::from_millis(configuration.limits.total_timeout_milliseconds);
        let mut hash_budget = configuration.limits.maximum_total_hash_bytes;
        let mut capture_total = 0_u64;
        let mut tools = Vec::new();
        for tool in &configuration.tools {
            let mut result = ToolchainProbeToolResult { dependency_id: tool.dependency_id.clone(), profile: tool.profile,
                status: ToolchainStatus::Unverified, before_sha256: None, after_sha256: None,
                commands: Vec::new(), version: None, observation: None, diagnostics: Vec::new() };
            for (id, arguments) in probe_commands(tool.profile) {
                if let Some(detail) = interrupted(cancellation, deadline) {
                    result.diagnostics.push(detail.to_owned()); break;
                }
                if capture_total >= configuration.limits.maximum_total_capture_bytes {
                    result.diagnostics.push("total capture budget exhausted before launch".to_owned()); break;
                }
                match hash_pin(tool, &configuration.limits, &mut hash_budget, deadline, cancellation) {
                    Ok(digest) => { if result.before_sha256.is_none() { result.before_sha256 = Some(digest); } }
                    Err(error) => {
                        result.status = error.status;
                        if result.before_sha256.is_none() { result.before_sha256 = error.observed; }
                        else { result.after_sha256 = error.observed; }
                        result.diagnostics.push(format!("before {id}: {}", error.detail)); break;
                    }
                }
                let command = run_command(tool, &id, &arguments, &configuration.limits, deadline, cancellation, &mut capture_total);
                let succeeded = command.outcome == ToolchainProbeCommandOutcome::Succeeded;
                if !succeeded {
                    result.status = if command.outcome == ToolchainProbeCommandOutcome::Failed { ToolchainStatus::Incompatible } else { ToolchainStatus::Unverified };
                    result.diagnostics.push(format!("command {id} did not complete successfully: {:?}", command.outcome));
                }
                result.commands.push(command);
                match hash_pin(tool, &configuration.limits, &mut hash_budget, deadline, cancellation) {
                    Ok(digest) => result.after_sha256 = Some(digest),
                    Err(error) => { result.status = error.status; result.after_sha256 = error.observed;
                        result.diagnostics.push(format!("after {id}: {}", error.detail)); break; }
                }
                if !succeeded { break; }
            }
            let command_set_complete = result.commands.len() == probe_commands(tool.profile).len()
                && result.commands.iter().all(|command| command.outcome == ToolchainProbeCommandOutcome::Succeeded)
                && result.before_sha256.as_deref() == Some(tool.expected_sha256.as_str())
                && result.after_sha256.as_deref() == Some(tool.expected_sha256.as_str())
                && result.diagnostics.is_empty();
            if command_set_complete {
                match parse_probe_observation(tool, &result.commands, &tool.expected_sha256) {
                    Ok(parsed) => {
                        result.status = if parsed.version.normalized_semver.is_some() {
                            ToolchainStatus::Installed
                        } else { ToolchainStatus::Unverified };
                        if parsed.version.normalized_semver.is_none() { result.diagnostics.push("raw version was retained without inventing a semantic version".to_owned()); }
                        result.version = Some(parsed.version); result.observation = Some(parsed.observation);
                    }
                    Err(error) => result.diagnostics.push(format!("successful process output could not be parsed as the fixed profile: {error}")),
                }
            }
            result.diagnostics = result.diagnostics.into_iter().map(bounded_detail).collect();
            tools.push(result);
        }
        let report = ToolchainProbeReport { schema: TOOLCHAIN_PROBE_REPORT_SCHEMA.to_owned(),
            request_sha256: configuration.sha256()?, configuration: configuration.clone(),
            complete: tools.iter().all(|tool| tool.status == ToolchainStatus::Installed),
            native_qualification: false, tools };
        report.validate()?;
        Ok(report)
    }

    #[cfg(all(test, target_os = "macos"))]
    mod tests {
        use super::*;

        #[test]
        fn late_exit_registration_preserves_inherited_pipes_until_cleanup() {
            // The long-lived synthetic writer is always in the guarded group;
            // this test never waits for its sleep to finish.
            let child = Command::new("/bin/sh")
                .args(["-c", "( /bin/sleep 60 ) & exit 0"])
                .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
                .process_group(0).spawn().unwrap();
            let mut group = ProcessGroup { child, may_signal: true, exit_observer: None };
            let mut first = macos::ExitObserver::register(group.child.id()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if matches!(first, macos::ExitObserver::ExitingBeforeRegistration)
                    || first.poll().unwrap().is_some() { break; }
                assert!(Instant::now() < deadline, "synthetic leader exit was not observed");
                thread::sleep(Duration::from_millis(1));
            }
            group.exit_observer = Some(macos::ExitObserver::register(group.child.id()).unwrap());
            assert!(matches!(group.exit_observer, Some(macos::ExitObserver::ExitingBeforeRegistration)));
            let mut output = group.child.stdout.take().unwrap();
            nonblocking(&output).unwrap();
            let mut capture = Capture::default();
            let mut total = 0;
            assert!(!poll_capture(&mut output, &mut capture, 1024, &mut total, 1024).unwrap());
            assert!(!capture.eof, "descendant must still own the inherited pipe");
            assert!(group.observe_exit(false).unwrap().is_none());
            assert!(group.may_signal, "ESRCH must not reap or terminate an open-pipe group");
            drop(output);
            assert_eq!(reap(&mut group, Duration::from_millis(20), true).unwrap().code(), Some(0));
            assert!(!group.may_signal);
        }
    }
}
