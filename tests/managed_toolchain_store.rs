use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use aniflow::toolchain::managed::*;

fn io(error: impl std::fmt::Display) -> aniflow::Error {
    aniflow::Error::new(aniflow::ErrorCategory::Io, error.to_string())
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct FixtureDownloader {
    bytes: BTreeMap<String, Vec<u8>>,
    calls: AtomicUsize,
    corrupt: bool,
    cancel: bool,
}
impl ManagedDownloader for FixtureDownloader {
    fn download(
        &self,
        artifact: &ManagedArtifact,
        destination: &Path,
        control: &ManagedControl,
    ) -> aniflow::Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut bytes = self.bytes[&artifact.id].clone();
        if self.corrupt {
            bytes[0] ^= 1;
        }
        fs::write(destination, bytes).map_err(io)?;
        if self.cancel {
            control.cancellation.cancel();
        }
        Ok(())
    }
}

fn fixture() -> (ManagedCatalog, FixtureDownloader) {
    let mut bytes = BTreeMap::new();
    let mut artifacts = Vec::new();
    for (id, path, role, value) in [
        (
            "processor",
            "bin/processor",
            ManagedFileRole::Executable,
            b"fixture executable".as_slice(),
        ),
        (
            "parameter",
            "models/anime.param",
            ManagedFileRole::ModelParameter,
            b"fixture parameter".as_slice(),
        ),
        (
            "weights",
            "models/anime.bin",
            ManagedFileRole::ModelWeights,
            b"fixture weights".as_slice(),
        ),
        (
            "license",
            "LICENSE",
            ManagedFileRole::License,
            b"fixture license".as_slice(),
        ),
    ] {
        bytes.insert(id.to_owned(), value.to_vec());
        artifacts.push(ManagedArtifact {
            id: id.to_owned(),
            url: format!("https://github.com/example/release/{id}"),
            sha256: sha(value),
            length: value.len() as u64,
            format: ManagedArtifactFormat::Raw,
            files: vec![ManagedFile {
                member: String::new(),
                path: path.to_owned(),
                sha256: sha(value),
                length: value.len() as u64,
                role,
                executable: role == ManagedFileRole::Executable,
            }],
        });
    }
    let package = ManagedPackage {
        id: "upscayl-fixture-v1-linux-x86_64-anime".to_owned(),
        provider: ManagedProvider::Upscayl,
        version: "fixture-v1".to_owned(),
        platform: ManagedPlatform {
            os: "linux".to_owned(),
            arch: "x86_64".to_owned(),
        },
        backend: "vulkan".to_owned(),
        source_revision: "fixture-source-revision".to_owned(),
        model_id: "anime".to_owned(),
        model_scale: 4,
        executable: "bin/processor".to_owned(),
        artifacts,
        licenses: vec![ManagedLicense {
            spdx: "MIT".to_owned(),
            path: "LICENSE".to_owned(),
            source_url: "https://github.com/example/source/LICENSE".to_owned(),
        }],
        notes: vec!["Test bytes are fixtures, not native qualification.".to_owned()],
    };
    (
        ManagedCatalog {
            schema: MANAGED_CATALOG_SCHEMA.to_owned(),
            revision: "fixture-v1".to_owned(),
            packages: vec![package],
        },
        FixtureDownloader {
            bytes,
            calls: AtomicUsize::new(0),
            corrupt: false,
            cancel: false,
        },
    )
}

fn zipped_fixture(extra: Option<(&str, bool)>) -> (ManagedCatalog, FixtureDownloader) {
    let (mut catalog, original) = fixture();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut files = Vec::new();
    for artifact in &catalog.packages[0].artifacts {
        let mut file = artifact.files[0].clone();
        file.member = format!("release/{}", file.path);
        zip.start_file(
            &file.member,
            SimpleFileOptions::default().unix_permissions(if file.executable {
                0o755
            } else {
                0o644
            }),
        )
        .unwrap();
        zip.write_all(&original.bytes[&artifact.id]).unwrap();
        files.push(file);
    }
    if let Some((name, symlink)) = extra {
        if symlink {
            zip.add_symlink(name, "../../escape", SimpleFileOptions::default())
                .unwrap();
        } else {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(b"unselected").unwrap();
        }
    }
    let bytes = zip.finish().unwrap().into_inner();
    catalog.packages[0].artifacts = vec![ManagedArtifact {
        id: "bundle".to_owned(),
        url: "https://github.com/example/release/bundle.zip".to_owned(),
        sha256: sha(&bytes),
        length: bytes.len() as u64,
        format: ManagedArtifactFormat::Zip,
        files,
    }];
    (
        catalog,
        FixtureDownloader {
            bytes: BTreeMap::from([("bundle".to_owned(), bytes)]),
            calls: AtomicUsize::new(0),
            corrupt: false,
            cancel: false,
        },
    )
}

fn store(temp: &tempfile::TempDir) -> ManagedStore {
    ManagedStore::new(temp.path().join("owned")).unwrap()
}

#[test]
fn planning_and_missing_offline_acquisition_have_no_side_effects() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    assert_eq!(
        store.plan(&catalog, id, true).unwrap().state,
        ManagedPlanState::Missing
    );
    assert!(!store.root().exists());
    let control = ManagedControl {
        offline: true,
        ..Default::default()
    };
    assert!(store.acquire(&catalog, id, &downloader, &control).is_err());
    assert!(!store.root().exists());
    assert_eq!(downloader.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn real_installed_bytes_receipt_and_cached_offline_reinstall() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    let installed = store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    assert_eq!(
        installed.receipt.package_sha256,
        package_sha256(&catalog.packages[0]).unwrap()
    );
    assert_eq!(
        fs::read(installed.executable()).unwrap(),
        b"fixture executable"
    );
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(store.plan(&catalog, id, false).unwrap().network_bytes, 0);
    store.uninstall(id).unwrap();
    assert_eq!(
        store.plan(&catalog, id, true).unwrap().state,
        ManagedPlanState::Cached
    );
    let control = ManagedControl {
        offline: true,
        ..Default::default()
    };
    store.acquire(&catalog, id, &downloader, &control).unwrap();
    assert_eq!(downloader.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn corrupt_download_never_publishes_a_ready_version() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, mut downloader) = fixture();
    downloader.corrupt = true;
    let id = &catalog.packages[0].id;
    assert!(
        store
            .acquire(&catalog, id, &downloader, &ManagedControl::default())
            .unwrap_err()
            .to_string()
            .contains("SHA-256")
    );
    assert!(!store.package_root(id).unwrap().exists());
    assert_eq!(
        fs::read_dir(store.root().join("staging")).unwrap().count(),
        0
    );
}

#[test]
fn cancelled_transport_retains_no_ready_or_partial_archive() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, mut downloader) = fixture();
    downloader.cancel = true;
    let id = &catalog.packages[0].id;
    assert!(
        store
            .acquire(&catalog, id, &downloader, &ManagedControl::default())
            .unwrap_err()
            .to_string()
            .contains("cancel")
    );
    assert!(!store.package_root(id).unwrap().exists());
    assert_eq!(
        fs::read_dir(store.root().join("archives")).unwrap().count(),
        0
    );
    downloader.cancel = false;
    store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
}

#[test]
fn selected_zip_members_are_verified_and_preserve_layout() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = zipped_fixture(Some(("release/unused-model.bin", false)));
    let id = &catalog.packages[0].id;
    let installed = store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    assert!(installed.root.join("models/anime.bin").exists());
    assert!(!installed.root.join("unused-model.bin").exists());
}

#[test]
fn even_unselected_zip_traversal_and_symlinks_are_rejected() {
    for extra in [
        ("../escape", false),
        ("release/escape-link", true),
        ("release/CON", false),
        ("release/Models/ANIME.BIN", false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        let (catalog, downloader) = zipped_fixture(Some(extra));
        let id = &catalog.packages[0].id;
        assert!(
            store
                .acquire(&catalog, id, &downloader, &ManagedControl::default())
                .is_err(),
            "{extra:?}"
        );
        assert!(!store.package_root(id).unwrap().exists());
        assert!(!temp.path().join("escape").exists());
    }
}

#[test]
fn catalog_refuses_paths_aliases_fake_digests_and_missing_models() {
    for bad_path in [
        "../escape",
        "/absolute",
        "a\\b",
        "AUX.bin",
        "a/../b",
        "a//b",
        "a/b.",
        "a:b",
    ] {
        let (mut catalog, _) = fixture();
        catalog.packages[0].artifacts[0].files[0].path = bad_path.to_owned();
        assert!(catalog.validate().is_err(), "{bad_path}");
    }
    let (mut catalog, _) = fixture();
    catalog.packages[0].artifacts[0].sha256 = "0".repeat(64);
    assert!(catalog.validate().is_err());
    let (mut catalog, _) = fixture();
    catalog.packages[0].artifacts[2].files[0].role = ManagedFileRole::Notice;
    assert!(catalog.validate().is_err());
    let (mut catalog, _) = fixture();
    catalog.packages[0].artifacts[1].files[0].path = "BIN/PROCESSOR".to_owned();
    assert!(catalog.validate().is_err());
}

#[test]
fn active_and_retained_versions_are_preserved_through_upgrade_and_rollback() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (mut catalog, downloader) = fixture();
    let first = catalog.packages[0].id.clone();
    let mut second_package = catalog.packages[0].clone();
    second_package.id = "upscayl-fixture-v2-linux-x86_64-anime".to_owned();
    second_package.version = "fixture-v2".to_owned();
    let second = second_package.id.clone();
    catalog.packages.push(second_package);
    store
        .upgrade(&catalog, &first, &downloader, &ManagedControl::default())
        .unwrap();
    store.retain(&first, "run-1").unwrap();
    assert!(store.uninstall(&first).is_err());
    store
        .upgrade(&catalog, &second, &downloader, &ManagedControl::default())
        .unwrap();
    assert!(store.package_root(&first).unwrap().exists());
    assert!(store.uninstall(&first).is_err());
    store.rollback(&catalog, &first).unwrap();
    assert_eq!(store.references().unwrap().active, vec![first.clone()]);
    store.uninstall(&second).unwrap();
    store.release_retention(&first, "run-1").unwrap();
    store.deactivate(&first).unwrap();
    store.uninstall(&first).unwrap();
}

#[test]
fn corrupt_package_requires_selected_repair_and_keeps_quarantine() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    let installed = store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    fs::write(installed.executable(), b"broken").unwrap();
    assert_eq!(
        store.plan(&catalog, id, false).unwrap().state,
        ManagedPlanState::Corrupt
    );
    assert!(
        store
            .acquire(&catalog, id, &downloader, &ManagedControl::default())
            .is_err()
    );
    store
        .repair(
            &catalog,
            id,
            &downloader,
            &ManagedControl {
                offline: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        fs::read_dir(store.root().join("quarantine"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(downloader.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn retention_prevents_corrupt_repair_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    let installed = store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    store.retain(id, "run-1").unwrap();
    fs::write(installed.executable(), b"broken").unwrap();
    assert!(
        store
            .repair(&catalog, id, &downloader, &ManagedControl::default())
            .is_err()
    );
    assert_eq!(fs::read(installed.executable()).unwrap(), b"broken");
}

#[test]
fn cache_corruption_is_not_offline_readiness() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    store.uninstall(id).unwrap();
    fs::write(
        store
            .root()
            .join("archives")
            .join(&catalog.packages[0].artifacts[0].sha256),
        b"corrupt",
    )
    .unwrap();
    let calls = downloader.calls.load(Ordering::SeqCst);
    assert!(
        store
            .acquire(
                &catalog,
                id,
                &downloader,
                &ManagedControl {
                    offline: true,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(calls, downloader.calls.load(Ordering::SeqCst));
    store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    assert_eq!(calls + 1, downloader.calls.load(Ordering::SeqCst));
}

#[test]
fn refuses_user_directories_unknown_files_and_repointed_ids() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (mut catalog, downloader) = fixture();
    let id = catalog.packages[0].id.clone();
    fs::create_dir(store.root()).unwrap();
    fs::write(store.root().join("personal-file"), b"mine").unwrap();
    assert!(
        store
            .acquire(&catalog, &id, &downloader, &ManagedControl::default())
            .is_err()
    );
    assert_eq!(
        fs::read(store.root().join("personal-file")).unwrap(),
        b"mine"
    );
    fs::remove_file(store.root().join("personal-file")).unwrap();
    let installed = store
        .acquire(&catalog, &id, &downloader, &ManagedControl::default())
        .unwrap();
    fs::write(installed.root.join("unknown"), b"mine").unwrap();
    assert!(store.uninstall(&id).is_err());
    assert!(installed.executable().exists());
    fs::remove_file(installed.root.join("unknown")).unwrap();
    catalog.packages[0]
        .notes
        .push("different immutable specification".to_owned());
    assert!(store.inspect(&catalog, &id).is_err());
    assert!(
        store
            .repair(&catalog, &id, &downloader, &ManagedControl::default())
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn symlink_root_and_model_are_rejected_without_touching_targets() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let foreign = temp.path().join("foreign");
    fs::create_dir(&foreign).unwrap();
    symlink(&foreign, temp.path().join("link")).unwrap();
    assert!(ManagedStore::new(temp.path().join("link")).is_err());
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    let installed = store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    let model = installed.root.join("models/anime.bin");
    fs::remove_file(&model).unwrap();
    fs::write(foreign.join("model"), b"personal").unwrap();
    symlink(foreign.join("model"), &model).unwrap();
    assert!(store.inspect(&catalog, id).is_err());
    assert!(store.uninstall(id).is_err());
    assert_eq!(fs::read(foreign.join("model")).unwrap(), b"personal");
}

#[test]
fn writer_lock_refuses_overlapping_acquisition() {
    struct Blocking {
        fixture: FixtureDownloader,
        entered: std::sync::mpsc::Sender<()>,
        proceed: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl ManagedDownloader for Blocking {
        fn download(
            &self,
            artifact: &ManagedArtifact,
            destination: &Path,
            control: &ManagedControl,
        ) -> aniflow::Result<()> {
            if self.fixture.calls.load(Ordering::SeqCst) == 0 {
                self.entered.send(()).unwrap();
                self.proceed.lock().unwrap().recv().unwrap();
            }
            self.fixture.download(artifact, destination, control)
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = catalog.packages[0].id.clone();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (proceed_tx, proceed_rx) = std::sync::mpsc::channel();
    let blocking = Arc::new(Blocking {
        fixture: downloader,
        entered: entered_tx,
        proceed: Mutex::new(proceed_rx),
    });
    let other_store = store.clone();
    let other_catalog = catalog.clone();
    let worker = Arc::clone(&blocking);
    let other_id = id.clone();
    let thread = std::thread::spawn(move || {
        other_store.acquire(
            &other_catalog,
            &other_id,
            worker.as_ref(),
            &ManagedControl::default(),
        )
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let error = store
        .acquire(&catalog, &id, blocking.as_ref(), &ManagedControl::default())
        .unwrap_err();
    assert!(error.to_string().contains("busy"));
    proceed_tx.send(()).unwrap();
    thread.join().unwrap().unwrap();
}

#[test]
fn compiled_upstream_catalog_is_closed_and_complete() {
    let catalog = bundled_catalog().unwrap();
    assert!(!catalog.packages.is_empty());
    assert!(
        catalog
            .packages
            .iter()
            .any(|package| package.provider == ManagedProvider::Rife)
    );
    assert!(
        catalog
            .packages
            .iter()
            .any(|package| package.provider == ManagedProvider::Upscayl)
    );
    for package in catalog.packages {
        assert!(
            package
                .files()
                .all(|file| !file.sha256.bytes().all(|byte| byte == b'0'))
        );
    }
}

#[test]
fn default_transport_offline_and_untrusted_origins_have_no_effects() {
    let temp = tempfile::tempdir().unwrap();
    let (catalog, _) = fixture();
    let mut artifact = catalog.packages[0].artifacts[0].clone();
    let destination = temp.path().join("download");
    assert!(
        HttpsDownloader
            .download(
                &artifact,
                &destination,
                &ManagedControl {
                    offline: true,
                    ..Default::default()
                }
            )
            .is_err()
    );
    artifact.url = "https://untrusted.example/private".to_owned();
    assert!(
        HttpsDownloader
            .download(&artifact, &destination, &ManagedControl::default())
            .is_err()
    );
    assert!(!destination.exists());
}

#[test]
fn undeclared_empty_directories_cannot_be_ready_or_deleted_as_owned_members() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    let installed = store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    fs::create_dir(installed.root.join("manual-empty-directory")).unwrap();
    assert_eq!(
        store.plan(&catalog, id, false).unwrap().state,
        ManagedPlanState::Corrupt
    );
    assert!(store.uninstall(id).is_err());
    assert!(installed.root.join("manual-empty-directory").is_dir());
    store
        .repair(
            &catalog,
            id,
            &downloader,
            &ManagedControl {
                offline: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!installed.root.join("manual-empty-directory").exists());
    assert_eq!(
        fs::read_dir(store.root().join("quarantine"))
            .unwrap()
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn hardlinked_managed_files_and_dangling_cache_links_are_refused() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    let installed = store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    let alias = temp.path().join("external-model-alias");
    fs::hard_link(installed.root.join("models/anime.bin"), &alias).unwrap();
    assert!(store.inspect(&catalog, id).is_err());
    assert!(store.uninstall(id).is_err());
    assert_eq!(fs::read(&alias).unwrap(), b"fixture weights");
    fs::remove_file(alias).unwrap();
    store.uninstall(id).unwrap();
    let archive = store
        .root()
        .join("archives")
        .join(&catalog.packages[0].artifacts[0].sha256);
    fs::remove_file(&archive).unwrap();
    symlink(temp.path().join("missing-user-target"), &archive).unwrap();
    assert!(
        store
            .acquire(&catalog, id, &downloader, &ManagedControl::default())
            .is_err()
    );
    assert!(
        fs::symlink_metadata(archive)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn dangling_reference_file_never_means_no_retained_runs() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let (catalog, downloader) = fixture();
    let id = &catalog.packages[0].id;
    let installed = store
        .acquire(&catalog, id, &downloader, &ManagedControl::default())
        .unwrap();
    store.retain(id, "run-1").unwrap();
    fs::remove_file(store.root().join("references.json")).unwrap();
    symlink(
        temp.path().join("missing-references"),
        store.root().join("references.json"),
    )
    .unwrap();
    assert!(store.references().is_err());
    assert!(store.uninstall(id).is_err());
    assert_eq!(
        fs::read(installed.executable()).unwrap(),
        b"fixture executable"
    );
    fs::write(installed.executable(), b"broken").unwrap();
    assert!(
        store
            .repair(&catalog, id, &downloader, &ManagedControl::default())
            .is_err()
    );
    assert_eq!(fs::read(installed.executable()).unwrap(), b"broken");
}

/// Explicit network qualification of only public managed acquisition. This
/// does not execute Upscayl, process a video or establish GPU/Mac qualification.
#[test]
#[ignore = "downloads the selected official public artifacts (~51 MB); run explicitly"]
fn actual_official_linux_upscayl_acquisition_and_offline_readback() {
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return;
    }
    let catalog = bundled_catalog().unwrap();
    let package = catalog
        .packages
        .iter()
        .find(|package| {
            package.provider == ManagedProvider::Upscayl
                && package.platform == ManagedPlatform::current()
                && package.model_id == "realesr-animevideov3"
        })
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let installed = store
        .acquire(
            &catalog,
            &package.id,
            &HttpsDownloader,
            &ManagedControl::default(),
        )
        .unwrap();
    assert_eq!(installed.receipt.package, *package);
    assert_eq!(installed.files(ManagedFileRole::ModelWeights).count(), 1);
    let offline = store
        .acquire(
            &catalog,
            &package.id,
            &HttpsDownloader,
            &ManagedControl {
                offline: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(installed.receipt, offline.receipt);
    println!(
        "actual_public_acquisition package={} package_sha256={} installed_bytes={} exact_offline_readback=true native_execution=false",
        package.id,
        installed.receipt.package_sha256,
        package.files().map(|file| file.length).sum::<u64>()
    );
}
