//! Synthetic file-closure safety tests; no native library or model is executed.
use aniflow::CancellationToken;
use aniflow::toolchain::managed_runtime::{ManagedRuntimeFile, ManagedRuntimePin};
use sha2::{Digest, Sha256};
use std::path::Path;
fn pin(directory: &Path) -> ManagedRuntimePin {
    let source = directory.join("original.dylib");
    std::fs::write(&source, b"fixture runtime library, never executed").unwrap();
    ManagedRuntimePin {
        package_id: "fixture-runtime".into(),
        package_sha256: "a".repeat(64),
        catalog_revision: "fixture-v1".into(),
        installation_roots: Vec::new(),
        runtime_files: vec![ManagedRuntimeFile {
            path: source,
            relative_path: "lib/libfixture.dylib".into(),
            sha256: format!(
                "{:x}",
                Sha256::digest(b"fixture runtime library, never executed")
            ),
            byte_count: 39,
        }],
    }
}
#[test]
fn exact_runtime_staging_membership_and_both_original_and_private_drift() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut pin = pin(&root);
    // Keep the fixture's byte declaration independent of its digest.
    pin.runtime_files[0].byte_count = std::fs::metadata(&pin.runtime_files[0].path).unwrap().len();
    let staged = root.join("staged");
    std::fs::create_dir(&staged).unwrap();
    pin.stage(&staged, &CancellationToken::default()).unwrap();
    pin.verify_membership(&staged, &[], &[]).unwrap();
    std::fs::write(staged.join("undeclared.txt"), b"extra").unwrap();
    assert!(pin.verify_membership(&staged, &[], &[]).is_err());
    std::fs::remove_file(staged.join("undeclared.txt")).unwrap();
    std::fs::write(
        staged.join("lib/libfixture.dylib"),
        b"changed private bytes",
    )
    .unwrap();
    assert!(pin.verify_staged(&staged).is_err());
    std::fs::write(&pin.runtime_files[0].path, b"changed original bytes").unwrap();
    assert!(pin.observed_bytes().is_err());
}
#[test]
fn runtime_paths_lengths_collisions_and_cancel_refuse() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let pin = pin(&root);
    for path in [
        "../lib.dylib",
        "/absolute.dylib",
        "lib/../../escape",
        "models/tool.dylib",
        "MODELS/tool.dylib",
        "FFMPEG",
        "processor",
        "lib/bad name.so",
    ] {
        let mut changed = pin.clone();
        changed.runtime_files[0].relative_path = path.into();
        assert!(changed.validate().is_err(), "{path}");
    }
    let mut duplicate = pin.clone();
    duplicate
        .runtime_files
        .push(duplicate.runtime_files[0].clone());
    assert!(duplicate.validate().is_err());
    let mut protected = pin.clone();
    protected.installation_roots = vec![root.join("installed")];
    assert!(
        protected
            .require_output_disjoint(&root.join("installed/run"))
            .is_err()
    );
    assert!(
        protected
            .require_output_disjoint(&root.join("installed"))
            .is_err()
    );
    protected.require_output_disjoint(&root).unwrap();
    assert!(
        protected.observed_bytes().is_err(),
        "missing installation root cannot be observed"
    );
    let mut case_collision = pin.clone();
    let mut alternate = case_collision.runtime_files[0].clone();
    alternate.relative_path = "LIB/libfixture.dylib".into();
    case_collision.runtime_files.push(alternate);
    assert!(case_collision.validate().is_err());
    let mut directory_collision = pin.clone();
    let mut alternate = directory_collision.runtime_files[0].clone();
    alternate.relative_path = "LIB/other.dylib".into();
    directory_collision.runtime_files.push(alternate);
    assert!(directory_collision.validate().is_err());
    let mut parent = pin.clone();
    let mut child = parent.runtime_files[0].clone();
    child.relative_path = "lib/libfixture.dylib/child".into();
    parent.runtime_files.push(child);
    assert!(parent.validate().is_err());
    let token = CancellationToken::default();
    token.cancel();
    assert!(pin.stage(&root, &token).is_err());
    let mut length = pin.clone();
    length.runtime_files[0].byte_count = 0;
    assert!(length.validate().is_err());
}
