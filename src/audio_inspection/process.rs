//! Bounded tool capture; provider children inherit the outer runtime's process group.
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::types::{
    AudioInspectionConfiguration, AudioInspectionDiagnostic, AudioInspectionDiagnosticCode as Code,
    AudioToolPin,
};
use crate::CancellationToken;

pub(super) enum GroupPolicy {
    Own,
    Inherit,
}
pub(super) struct ToolCapture {
    pub stdout: Vec<u8>,
}
pub(super) type ToolResult<T> = std::result::Result<T, AudioInspectionDiagnostic>;

pub(super) fn failure(code: Code, tool: &str, message: &str) -> AudioInspectionDiagnostic {
    AudioInspectionDiagnostic {
        code,
        tool: tool.to_owned(),
        message: message.to_owned(),
    }
}

pub(super) fn hash_regular(path: &Path, maximum: u64) -> std::io::Result<(String, u64)> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > maximum {
        return Err(std::io::Error::other(
            "expected a bounded nonsymlink regular file",
        ));
    }
    let mut input = File::open(path)?.take(maximum + 1);
    let mut digest = Sha256::new();
    let mut count = 0;
    let mut buffer = [0_u8; 65536];
    loop {
        let size = input.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        count += size as u64;
        if count > maximum {
            return Err(std::io::Error::other("file exceeds its bound"));
        }
        digest.update(&buffer[..size]);
    }
    if count != metadata.len() {
        return Err(std::io::Error::other("file changed while hashing"));
    }
    Ok((format!("{:x}", digest.finalize()), count))
}

pub(super) fn verify_pin(pin: &AudioToolPin, id: &str) -> ToolResult<()> {
    let (digest, _) = hash_regular(&pin.executable, 512 * 1024 * 1024).map_err(|error| {
        let code = if error.kind() == std::io::ErrorKind::NotFound {
            Code::MissingTool
        } else {
            Code::InvalidTool
        };
        failure(
            code,
            id,
            "configured executable is missing, unreadable, not a regular file, or exceeds 512 MiB",
        )
    })?;
    if digest != pin.sha256 {
        return Err(failure(
            Code::ToolDigestMismatch,
            id,
            "configured executable SHA-256 changed",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&pin.executable)
            .map_err(|_| {
                failure(
                    Code::InvalidTool,
                    id,
                    "cannot inspect executable permissions",
                )
            })?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err(failure(
                Code::InvalidTool,
                id,
                "configured tool is not executable",
            ));
        }
    }
    Ok(())
}

fn capture(
    mut input: impl Read + Send + 'static,
    maximum: u64,
) -> Receiver<std::io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut output = Vec::new();
        let result = input
            .by_ref()
            .take(maximum + 1)
            .read_to_end(&mut output)
            .map(|_| output);
        let _ = sender.send(result);
    });
    receiver
}

#[cfg(unix)]
fn stop(child: &mut Child, policy: &GroupPolicy) {
    if matches!(policy, GroupPolicy::Own) {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
fn stop(child: &mut Child, _policy: &GroupPolicy) {
    let _ = child.kill();
    let _ = child.wait();
}

pub(super) fn run_tool(
    pin: &AudioToolPin,
    id: &str,
    arguments: &[OsString],
    config: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
    policy: GroupPolicy,
    directory: Option<&Path>,
) -> ToolResult<ToolCapture> {
    if !cfg!(unix) {
        return Err(failure(
            Code::UnsupportedPlatform,
            id,
            "this adapter requires Unix process-group cancellation",
        ));
    }
    if cancellation.is_cancelled() {
        return Err(failure(
            Code::Cancelled,
            id,
            "inspection was cancelled before tool launch",
        ));
    }
    verify_pin(pin, id)?;
    let mut command = Command::new(&pin.executable);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("FFREPORT")
        .env_remove("AV_LOG_FORCE_COLOR")
        .env_remove("AV_LOG_FORCE_NOCOLOR");
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        if matches!(policy, GroupPolicy::Own) {
            command.process_group(0);
        }
    }
    let mut child = command.spawn().map_err(|_| {
        failure(
            Code::InvalidTool,
            id,
            "configured tool could not be launched",
        )
    })?;
    let stdout = capture(
        child.stdout.take().expect("piped stdout"),
        config.maximum_tool_output_bytes,
    );
    let stderr = capture(
        child.stderr.take().expect("piped stderr"),
        config.maximum_tool_output_bytes,
    );
    let deadline = Instant::now() + Duration::from_millis(config.tool_timeout_milliseconds);
    let mut captured_stdout = None;
    let mut captured_stderr = None;
    let mut status = None;
    loop {
        let reason = if cancellation.is_cancelled() {
            Some((Code::Cancelled, "inspection cancelled"))
        } else if Instant::now() >= deadline {
            Some((Code::ToolTimeout, "tool exceeded the configured deadline"))
        } else {
            None
        };
        if let Some((code, message)) = reason {
            stop(&mut child, &policy);
            return Err(failure(code, id, message));
        }
        for (receiver, captured) in [
            (&stdout, &mut captured_stdout),
            (&stderr, &mut captured_stderr),
        ] {
            if captured.is_none() {
                match receiver.try_recv() {
                    Ok(Ok(bytes)) if bytes.len() as u64 <= config.maximum_tool_output_bytes => {
                        *captured = Some(bytes)
                    }
                    Ok(Ok(_)) => {
                        stop(&mut child, &policy);
                        return Err(failure(
                            Code::ToolOutputLimit,
                            id,
                            "tool exceeded a configured output capture bound",
                        ));
                    }
                    Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                        stop(&mut child, &policy);
                        return Err(failure(Code::ToolFailed, id, "tool output capture failed"));
                    }
                    Err(mpsc::TryRecvError::Empty) => (),
                }
            }
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(_) => {
                    stop(&mut child, &policy);
                    return Err(failure(
                        Code::ToolFailed,
                        id,
                        "tool status observation failed",
                    ));
                }
            }
        }
        if let Some(exit) = status {
            if !exit.success() {
                stop(&mut child, &policy);
                return Err(failure(Code::ToolFailed, id, "tool exited unsuccessfully"));
            }
            if let (Some(stdout), Some(_)) = (&captured_stdout, &captured_stderr) {
                if matches!(policy, GroupPolicy::Own) {
                    stop(&mut child, &policy);
                }
                verify_pin(pin, id)?;
                return Ok(ToolCapture {
                    stdout: stdout.clone(),
                });
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}
