use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::catalog::https_url;
use super::*;

/// A caller-supplied transport is an acquisition adapter, never integrity
/// authority. The store always checks exact artifact length and SHA-256 again.
pub trait ManagedDownloader {
    fn download(
        &self,
        artifact: &ManagedArtifact,
        destination: &Path,
        control: &ManagedControl,
    ) -> crate::Result<()>;
}

/// Direct, bounded public HTTPS curl adapter. No shell or credentials are used.
/// Redirects are followed individually only after checking the destination host.
#[derive(Debug, Clone, Copy, Default)]
pub struct HttpsDownloader;

impl ManagedDownloader for HttpsDownloader {
    fn download(
        &self,
        artifact: &ManagedArtifact,
        destination: &Path,
        control: &ManagedControl,
    ) -> crate::Result<()> {
        control.check()?;
        if control.offline {
            return Err(dependency("offline managed acquisition refuses transport"));
        }
        https_url(&artifact.url)?;
        super::store::safe_path(destination)?;
        if destination.exists() {
            return Err(dependency("managed download destination must be absent"));
        }
        let started = Instant::now();
        let mut url = artifact.url.clone();
        for redirect in 0..=4 {
            control.check()?;
            https_url(&url)?;
            let remaining = Duration::from_secs(control.timeout_seconds)
                .checked_sub(started.elapsed())
                .ok_or_else(|| dependency("managed HTTPS acquisition timed out"))?;
            let parent = destination
                .parent()
                .ok_or_else(|| dependency("managed download parent is absent"))?;
            let header = tempfile::NamedTempFile::new_in(parent).map_err(io)?;
            let output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)
                .map_err(io)?;
            let mut command = Command::new("curl");
            command
                .args([
                    "--disable",
                    "--silent",
                    "--show-error",
                    "--fail",
                    "--proto",
                    "=https",
                    "--proto-redir",
                    "=https",
                    "--max-redirs",
                    "0",
                    "--connect-timeout",
                    "30",
                    "--retry",
                    "0",
                    "--max-time",
                ])
                .arg(remaining.as_secs_f64().to_string())
                .args([
                    "--max-filesize",
                    &artifact.length.to_string(),
                    "--dump-header",
                ])
                .arg(header.path())
                .arg("--url")
                .arg(&url)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let mut child = command.spawn().map_err(|error| {
                dependency(format!(
                    "managed HTTPS downloader needs curl on PATH: {error}"
                ))
            })?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| dependency("managed HTTPS pipe is absent"))?;
            let artifact_id = artifact.id.clone();
            let maximum = artifact.length;
            let observer = control.clone();
            let reader = std::thread::spawn(move || {
                copy_bounded(stdout, output, maximum, &artifact_id, &observer)
            });
            let result = loop {
                let headers_exceeded = header
                    .as_file()
                    .metadata()
                    .is_ok_and(|metadata| metadata.len() > 64 * 1024);
                if control.cancellation.is_cancelled()
                    || started.elapsed() >= Duration::from_secs(control.timeout_seconds)
                    || headers_exceeded
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(dependency(if headers_exceeded {
                        "managed HTTPS headers exceed byte budget"
                    } else if control.cancellation.is_cancelled() {
                        "managed HTTPS acquisition cancelled"
                    } else {
                        "managed HTTPS acquisition timed out"
                    }));
                }
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => break Ok(()),
                    Ok(Some(status)) => {
                        break Err(dependency(format!(
                            "managed HTTPS curl failed ({status}); exact public artifact remains unavailable"
                        )));
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                    Err(error) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break Err(io(error));
                    }
                }
            };
            let copy_result = reader
                .join()
                .map_err(|_| dependency("managed HTTPS reader failed"))?;
            if let Err(error) = result.and(copy_result) {
                let _ = std::fs::remove_file(destination);
                return Err(error);
            }
            if header.as_file().metadata().map_err(io)?.len() > 64 * 1024 {
                let _ = std::fs::remove_file(destination);
                return Err(dependency("managed HTTPS headers exceed byte budget"));
            }
            let text = std::fs::read_to_string(header.path()).map_err(io)?;
            let mut status = 0_u16;
            let mut location = None;
            for line in text.lines() {
                if line.starts_with("HTTP/") {
                    status = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0);
                    location = None;
                } else if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("location") {
                        location = Some(value.trim().to_owned());
                    }
                }
            }
            if status == 200 {
                return Ok(());
            }
            std::fs::remove_file(destination).map_err(io)?;
            if ![301, 302, 303, 307, 308].contains(&status) || redirect == 4 {
                return Err(dependency(format!(
                    "managed HTTPS unexpected status or redirect limit: {status}"
                )));
            }
            let next =
                location.ok_or_else(|| dependency("managed HTTPS redirect lacks location"))?;
            // Upstream release redirects are absolute. Reject relative/ambiguous
            // redirects rather than add URL interpretation or credential behavior.
            https_url(&next)?;
            url = next;
        }
        Err(dependency("managed HTTPS redirect limit reached"))
    }
}

fn copy_bounded(
    mut source: impl Read,
    mut destination: File,
    maximum: u64,
    artifact_id: &str,
    control: &ManagedControl,
) -> crate::Result<()> {
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        control.check()?;
        let count = source.read(&mut buffer).map_err(io)?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .ok_or_else(|| dependency("managed HTTPS byte count overflow"))?;
        if bytes > maximum {
            return Err(dependency(
                "managed HTTPS body exceeds exact artifact byte budget",
            ));
        }
        destination.write_all(&buffer[..count]).map_err(io)?;
        control.emit(ManagedProgress::DownloadProgress {
            artifact_id: artifact_id.to_owned(),
            bytes,
            total: maximum,
        });
    }
    destination.sync_all().map_err(io)
}
