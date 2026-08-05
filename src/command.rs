use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

pub fn doctor(required_commands: &[String]) -> Result<()> {
    println!("aniflow doctor");
    println!();

    let mut missing = Vec::new();
    for executable in required_commands {
        match executable_summary(executable) {
            Ok(version) => println!("  {executable:<24} ok  {version}"),
            Err(_) => {
                println!("  {executable:<24} missing");
                missing.push(executable.clone());
            }
        }
    }

    if missing.is_empty() {
        println!();
        println!("ready");
        Ok(())
    } else {
        bail!(
            "install the missing runtime dependencies: {}",
            missing.join(", ")
        )
    }
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
