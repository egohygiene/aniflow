//! A small pinned dispatcher for synthetic provider-protocol fixtures only.
//!
//! Planning hundreds of providers must not repeatedly hash the large compiled
//! test executable. The registry pins this script; on every actual invocation
//! it independently verifies the expected compiled binary bytes, then execs the
//! real CLI with unchanged arguments and process identity. This tests production
//! dispatch/acceptance, not native model behavior or executable-authenticity
//! qualification. Hash-tool and wrapper work are not throughput measurements.
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static COMPILED_PROVIDER: OnceLock<(PathBuf, String)> = OnceLock::new();
fn quoted(path: &Path) -> String {
    format!(
        "'{}'",
        path.to_str()
            .expect("UTF-8 test path")
            .replace('\'', "'\\''")
    )
}

pub fn write(root: &Path) -> PathBuf {
    let (binary, expected) = COMPILED_PROVIDER.get_or_init(|| {
        let binary = Path::new(env!("CARGO_BIN_EXE_aniflow"))
            .canonicalize()
            .unwrap();
        let expected = format!("{:x}", Sha256::digest(std::fs::read(&binary).unwrap()));
        (binary, expected)
    });
    // GNU/Linux and macOS runners supply these standard checksum utilities.
    // They run only in the controlled fixture, never inside product code.
    let (checker, flags) = [
        ("/usr/bin/sha256sum", ""),
        ("/bin/sha256sum", ""),
        ("/usr/bin/shasum", "-a 256"),
    ]
    .into_iter()
    .find(|(path, _)| Path::new(path).is_file())
    .expect("synthetic dispatcher requires sha256sum or shasum");
    let checksum = format!("{} {flags} {}", quoted(Path::new(checker)), quoted(binary));
    let script = format!(
        r#"#!/bin/sh
set -eu
# Synthetic dispatcher; the registry identity is this script, not a native-tool qualification.
observed=$({checksum})
[ "${{observed%% *}}" = "{expected}" ] || exit 97
exec {binary} "$@"
"#,
        binary = quoted(binary)
    );
    let path = root.join("pinned production CLI dispatcher");
    std::fs::write(&path, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}
