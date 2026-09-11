use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, Copy)]
pub struct ProcessLimits {
    pub timeout: Duration,
    pub maximum_output_bytes: usize,
}

#[derive(Debug)]
pub struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub duration: Duration,
}

/// Run a child directly, without a shell, while bounding runtime and captured output.
pub fn run_bounded<I, S>(
    program: &str,
    arguments: I,
    limits: ProcessLimits,
    cancelled: Option<Arc<AtomicBool>>,
) -> Result<BoundedOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let arguments = arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_owned())
        .collect::<Vec<_>>();
    let mut child = Command::new(program)
        .args(&arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to execute `{program}`"))?;
    let stdout = child
        .stdout
        .take()
        .context("child stdout pipe unavailable")?;
    let stderr = child
        .stderr
        .take()
        .context("child stderr pipe unavailable")?;
    let maximum_output_bytes = limits.maximum_output_bytes;
    let stdout_reader = thread::spawn(move || read_bounded(stdout, maximum_output_bytes));
    let stderr_reader = thread::spawn(move || read_bounded(stderr, maximum_output_bytes));
    let started = Instant::now();
    let status = loop {
        if cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            child.kill().ok();
            child.wait().ok();
            join_reader(stdout_reader)?;
            join_reader(stderr_reader)?;
            bail!("command `{program}` was cancelled");
        }
        if started.elapsed() >= limits.timeout {
            child.kill().ok();
            child.wait().ok();
            join_reader(stdout_reader)?;
            join_reader(stderr_reader)?;
            bail!(
                "command `{program}` exceeded its {} second timeout",
                limits.timeout.as_secs()
            );
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(25));
    };
    let (stdout, stdout_truncated) = join_reader(stdout_reader)?;
    let (stderr, stderr_truncated) = join_reader(stderr_reader)?;
    Ok(BoundedOutput {
        status,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
        duration: started.elapsed(),
    })
}

fn read_bounded(mut reader: impl Read, limit: usize) -> (Vec<u8>, bool) {
    let mut retained = Vec::with_capacity(limit.min(64 * 1024));
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let remaining = limit.saturating_sub(retained.len());
        let keep = remaining.min(read);
        retained.extend_from_slice(&buffer[..keep]);
        truncated |= keep < read;
    }
    (retained, truncated)
}

fn join_reader(reader: thread::JoinHandle<(Vec<u8>, bool)>) -> Result<(Vec<u8>, bool)> {
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("child output reader panicked"))
}

pub fn require_executable(executable: &str) -> Result<()> {
    let output = Command::new(executable)
        .arg("-version")
        .output()
        .with_context(|| format!("required executable `{executable}` was not found"))?;

    if !output.status.success() {
        bail!("required executable `{executable}` is not runnable");
    }

    Ok(())
}

pub fn require_available(executable: &str) -> Result<()> {
    Command::new(executable)
        .arg("--help")
        .output()
        .with_context(|| format!("required executable `{executable}` was not found"))?;
    Ok(())
}

pub fn run_logged<I, S>(program: &str, arguments: I, log_path: &Path) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let arguments = arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_owned())
        .collect::<Vec<_>>();

    let rendered = std::iter::once(program.to_owned())
        .chain(
            arguments
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned()),
        )
        .collect::<Vec<_>>()
        .join(" ");

    let output = Command::new(program)
        .args(&arguments)
        .output()
        .with_context(|| format!("failed to execute `{program}`"))?;

    let mut log = format!("$ {rendered}\n\n");
    log.push_str("--- stdout ---\n");
    log.push_str(&String::from_utf8_lossy(&output.stdout));
    log.push_str("\n--- stderr ---\n");
    log.push_str(&String::from_utf8_lossy(&output.stderr));
    std::fs::write(log_path, log)
        .with_context(|| format!("failed to write log {}", log_path.display()))?;

    if !output.status.success() {
        bail!("command `{program}` failed; see {}", log_path.display());
    }

    Ok(output)
}

fn version_line(executable: &str) -> Result<String> {
    let output = Command::new(executable)
        .arg("-version")
        .output()
        .with_context(|| format!("unable to run `{executable}`"))?;

    if !output.status.success() {
        bail!("`{executable}` returned a failure status");
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("version unavailable")
        .to_owned())
}

pub fn executable_summary(executable: &str) -> Result<String> {
    if matches!(executable, "ffmpeg" | "ffprobe") {
        return version_line(executable);
    }

    let output = Command::new(executable)
        .arg("--help")
        .output()
        .with_context(|| format!("unable to run `{executable}`"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let summary = stdout
        .lines()
        .chain(stderr.lines())
        .find(|line| !line.trim().is_empty())
        .unwrap_or("executable available")
        .trim()
        .to_owned();
    Ok(summary)
}
