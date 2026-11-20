use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::catalog::{MAXIMUM_ARCHIVE_MEMBERS, MAXIMUM_INSTALLED_BYTES, relative};
use super::*;

pub(crate) fn verify_file(
    path: &Path,
    expected_sha256: &str,
    expected_length: u64,
) -> crate::Result<()> {
    super::store::safe_path(path)?;
    let metadata = fs::symlink_metadata(path).map_err(io)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != expected_length
    {
        return Err(dependency(format!(
            "managed file length/type mismatch: {}",
            path.display()
        )));
    }
    let mut source = File::open(path).map_err(io)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let remaining = expected_length
            .saturating_sub(total)
            .saturating_add(1)
            .min(buffer.len() as u64) as usize;
        let count = source.read(&mut buffer[..remaining]).map_err(io)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > expected_length {
            return Err(dependency(
                "managed file grew beyond its declared verification budget",
            ));
        }
        digest.update(&buffer[..count]);
    }
    if total != expected_length || format!("{:x}", digest.finalize()) != expected_sha256 {
        return Err(dependency(format!(
            "managed SHA-256 mismatch: {}",
            path.display()
        )));
    }
    Ok(())
}

fn copy_member(
    reader: &mut dyn Read,
    file: &ManagedFile,
    stage: &Path,
    control: &ManagedControl,
) -> crate::Result<()> {
    let path = stage.join(&file.path);
    let parent = path
        .parent()
        .ok_or_else(|| dependency("managed member parent is absent"))?;
    super::store::safe_path(parent)?;
    fs::create_dir_all(parent).map_err(io)?;
    super::store::safe_path(parent)?;
    let mut target = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(io)?;
    let mut total = 0_u64;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        control.check()?;
        let count = reader.read(&mut buffer).map_err(io)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| dependency("managed extraction byte overflow"))?;
        if total > file.length {
            return Err(dependency("managed member exceeds declared byte count"));
        }
        target.write_all(&buffer[..count]).map_err(io)?;
        hash.update(&buffer[..count]);
    }
    if total != file.length || format!("{:x}", hash.finalize()) != file.sha256 {
        return Err(dependency(
            "managed extracted member digest/length mismatch",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        target
            .set_permissions(fs::Permissions::from_mode(if file.executable {
                0o755
            } else {
                0o644
            }))
            .map_err(io)?;
    }
    target.sync_all().map_err(io)?;
    control.emit(ManagedProgress::FileInstalled {
        path: file.path.clone(),
        bytes: total,
    });
    Ok(())
}

pub(crate) fn extract(
    artifact: &ManagedArtifact,
    archive: &Path,
    stage: &Path,
    control: &ManagedControl,
) -> crate::Result<()> {
    match artifact.format {
        ManagedArtifactFormat::Raw => copy_member(
            &mut File::open(archive).map_err(io)?,
            &artifact.files[0],
            stage,
            control,
        ),
        ManagedArtifactFormat::Zip => {
            let mut zip = zip::ZipArchive::new(File::open(archive).map_err(io)?).map_err(io)?;
            if zip.len() > MAXIMUM_ARCHIVE_MEMBERS {
                return Err(dependency("managed ZIP exceeds member budget"));
            }
            let mut paths = BTreeSet::new();
            let mut total = 0_u64;
            // Validate every member before writing anything, including unselected members.
            for index in 0..zip.len() {
                control.check()?;
                let member = zip.by_index(index).map_err(io)?;
                let name = member.name().trim_end_matches('/');
                relative(name)?;
                if !paths.insert(name.to_ascii_lowercase()) {
                    return Err(dependency("duplicate/case-colliding managed ZIP member"));
                }
                if member.enclosed_name().is_none() {
                    return Err(dependency("managed ZIP member escapes archive"));
                }
                if let Some(mode) = member.unix_mode() {
                    let kind = mode & 0o170000;
                    if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                        return Err(dependency(
                            "managed ZIP links/devices/special files are forbidden",
                        ));
                    }
                }
                total = total
                    .checked_add(member.size())
                    .ok_or_else(|| dependency("managed ZIP byte budget overflow"))?;
                if total > MAXIMUM_INSTALLED_BYTES {
                    return Err(dependency(
                        "managed ZIP exceeds total uncompressed byte budget",
                    ));
                }
            }
            for file in &artifact.files {
                control.check()?;
                let mut member = zip.by_name(&file.member).map_err(io)?;
                if member.is_dir() || member.size() != file.length {
                    return Err(dependency(
                        "managed ZIP selected member length/type mismatch",
                    ));
                }
                copy_member(&mut member, file, stage, control)?;
            }
            Ok(())
        }
    }
}
