//! Resolver policy fixtures; these are not native model/platform qualification.
use aniflow::managed_experience::*;
use aniflow::toolchain::managed::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
struct Downloads {
    files: BTreeMap<String, Vec<u8>>,
    calls: AtomicUsize,
}
impl ManagedDownloader for Downloads {
    fn download(
        &self,
        artifact: &ManagedArtifact,
        destination: &Path,
        _: &ManagedControl,
    ) -> aniflow::Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        fs::write(destination, &self.files[&artifact.id])
            .map_err(|error| aniflow::Error::new(aniflow::ErrorCategory::Io, error.to_string()))
    }
}
fn fixture() -> (ManagedCatalog, Downloads) {
    let mut files = BTreeMap::new();
    let mut artifacts = Vec::new();
    for (id, path, role, bytes) in [
        (
            "exe",
            "upscayl-bin",
            ManagedFileRole::Executable,
            b"synthetic executable".as_slice(),
        ),
        (
            "param",
            "models/anime.param",
            ManagedFileRole::ModelParameter,
            b"synthetic parameter".as_slice(),
        ),
        (
            "weights",
            "models/anime.bin",
            ManagedFileRole::ModelWeights,
            b"synthetic weights".as_slice(),
        ),
        (
            "license",
            "LICENSE",
            ManagedFileRole::License,
            b"synthetic license".as_slice(),
        ),
    ] {
        files.insert(id.to_owned(), bytes.to_vec());
        artifacts.push(ManagedArtifact {
            id: id.to_owned(),
            url: format!("https://github.com/example/releases/{id}"),
            sha256: sha(bytes),
            length: bytes.len() as u64,
            format: ManagedArtifactFormat::Raw,
            files: vec![ManagedFile {
                member: String::new(),
                path: path.to_owned(),
                sha256: sha(bytes),
                length: bytes.len() as u64,
                role,
                executable: role == ManagedFileRole::Executable,
            }],
        });
    }
    (
        ManagedCatalog {
            schema: MANAGED_CATALOG_SCHEMA.to_owned(),
            revision: "fixture-v1".to_owned(),
            packages: vec![ManagedPackage {
                id: "upscayl-fixture-anime".to_owned(),
                provider: ManagedProvider::Upscayl,
                version: "1.0.0".to_owned(),
                platform: ManagedPlatform::current(),
                backend: "vulkan".to_owned(),
                source_revision: "synthetic-source-only".to_owned(),
                model_id: "anime".to_owned(),
                model_scale: 4,
                executable: "upscayl-bin".to_owned(),
                artifacts,
                licenses: vec![ManagedLicense {
                    spdx: "MIT".to_owned(),
                    path: "LICENSE".to_owned(),
                    source_url: "https://github.com/example/source/LICENSE".to_owned(),
                }],
                notes: vec!["Fixture only".to_owned()],
            }],
        },
        Downloads {
            files,
            calls: AtomicUsize::new(0),
        },
    )
}
fn options(root: &Path) -> ManagedOptions {
    ManagedOptions {
        cache_directory: Some(root.join("managed")),
        ..ManagedOptions::default()
    }
}
fn write_exe(path: &Path) {
    fs::write(path, b"manual fixture executable").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
#[test]
fn planning_does_not_create_store_and_unselected_rife_has_no_acquisition() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (catalog, downloads) = fixture();
    let options = options(root);
    let plan = plan_experience(&catalog, &options, true, false).unwrap();
    assert!(!plan.ready);
    assert_eq!(plan.tools.len(), 1);
    assert_eq!(plan.tools[0].provider, ManagedProvider::Upscayl);
    assert!(!root.join("managed").exists());
    assert_eq!(downloads.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn acquisition_freezes_exact_paths_hashes_receipt_and_retention() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (catalog, downloads) = fixture();
    let options = options(root);
    let resolved = resolve_tool(
        &catalog,
        &options,
        ManagedProvider::Upscayl,
        &downloads,
        &ManagedControl::default(),
    )
    .unwrap();
    assert_eq!(resolved.executable_sha256, sha(&downloads.files["exe"]));
    assert_eq!(resolved.param_sha256, sha(&downloads.files["param"]));
    assert_eq!(
        resolved.package_id.as_deref(),
        Some("upscayl-fixture-anime")
    );
    assert_eq!(resolved.installation_roots.len(), 2);
    let reference =
        retain_native_configuration(&options, &resolved, b"exact synthetic native configuration")
            .unwrap()
            .unwrap();
    let store = ManagedStore::new(root.join("managed")).unwrap();
    assert!(
        store
            .references()
            .unwrap()
            .retained
            .iter()
            .any(|entry| entry.run_id == reference)
    );
    assert!(store.uninstall("upscayl-fixture-anime").is_err());
    let count = downloads.calls.load(Ordering::SeqCst);
    let second = resolve_tool(
        &catalog,
        &ManagedOptions {
            offline: true,
            ..options.clone()
        },
        ManagedProvider::Upscayl,
        &downloads,
        &ManagedControl::default(),
    )
    .unwrap();
    assert_eq!(resolved, second);
    assert_eq!(downloads.calls.load(Ordering::SeqCst), count);
}
#[test]
fn missing_explicit_executable_refuses_before_managed_acquisition() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (catalog, downloads) = fixture();
    let mut options = options(root);
    options.upscayl.executable = Some(root.join("missing"));
    options.upscayl.version = Some("2.15.0".to_owned());
    assert!(
        resolve_tool(
            &catalog,
            &options,
            ManagedProvider::Upscayl,
            &downloads,
            &ManagedControl::default()
        )
        .is_err()
    );
    assert_eq!(downloads.calls.load(Ordering::SeqCst), 0);
    assert!(!root.join("managed").exists());
}
#[test]
fn offline_missing_refuses_without_invoking_transport() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (catalog, downloads) = fixture();
    let options = ManagedOptions {
        offline: true,
        ..options(root)
    };
    assert!(
        resolve_tool(
            &catalog,
            &options,
            ManagedProvider::Upscayl,
            &downloads,
            &ManagedControl::default()
        )
        .is_err()
    );
    assert_eq!(downloads.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn manual_executable_and_managed_models_remain_independent() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (catalog, downloads) = fixture();
    let manual = root.join("manual-exe");
    write_exe(&manual);
    let mut options = options(root);
    options.upscayl.executable = Some(manual.clone());
    options.upscayl.version = Some("2.15.0".to_owned());
    let resolved = resolve_tool(
        &catalog,
        &options,
        ManagedProvider::Upscayl,
        &downloads,
        &ManagedControl::default(),
    )
    .unwrap();
    assert_eq!(resolved.executable, manual);
    assert_eq!(resolved.version, "2.15.0");
    assert!(resolved.param_path.starts_with(root.join("managed")));
    assert!(resolved.runtime_files.is_empty());
    assert_eq!(fs::read(&manual).unwrap(), b"manual fixture executable");
}
#[test]
fn all_manual_works_without_catalog_or_store_and_without_receipt_claim() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (mut catalog, downloads) = fixture();
    catalog.packages.clear();
    let manual = root.join("manual-exe");
    write_exe(&manual);
    fs::create_dir(root.join("models")).unwrap();
    fs::write(root.join("models/custom.param"), b"param").unwrap();
    fs::write(root.join("models/custom.bin"), b"weights").unwrap();
    let mut options = options(root);
    options.upscayl = ManagedToolOptions {
        executable: Some(manual),
        version: Some("2.15.0".to_owned()),
        model: Some("custom".to_owned()),
        model_directory: Some(root.join("models")),
        ..ManagedToolOptions::default()
    };
    let resolved = resolve_tool(
        &catalog,
        &options,
        ManagedProvider::Upscayl,
        &downloads,
        &ManagedControl::default(),
    )
    .unwrap();
    assert!(resolved.package_id.is_none());
    assert!(resolved.installation_roots.is_empty());
    assert!(!root.join("managed").exists());
    assert_eq!(downloads.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn precedence_applies_model_directory_to_both_roles_at_that_layer() {
    let user = ManagedToolOptions {
        model_param: Some("/user/old.param".into()),
        model_weights: Some("/user/old.bin".into()),
        executable: Some("/user/exe".into()),
        ..ManagedToolOptions::default()
    };
    let project = ManagedToolOptions {
        model_directory: Some("/project/models".into()),
        ..ManagedToolOptions::default()
    };
    let command = ManagedToolOptions {
        model_param: Some("/command/new.param".into()),
        ..ManagedToolOptions::default()
    };
    let effective = command.over(&project.over(&user));
    assert_eq!(effective.executable, user.executable);
    assert_eq!(effective.model_directory, project.model_directory);
    assert_eq!(effective.model_param, command.model_param);
    assert_eq!(effective.model_weights, None);
}
#[test]
fn generated_source_contract_preserves_closed_native_pins_and_optional_rife() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (catalog, downloads) = fixture();
    let options = options(root);
    let tool = resolve_tool(
        &catalog,
        &options,
        ManagedProvider::Upscayl,
        &downloads,
        &ManagedControl::default(),
    )
    .unwrap();
    let ffmpeg = root.join("ffmpeg");
    let ffprobe = root.join("ffprobe");
    write_exe(&ffmpeg);
    write_exe(&ffprobe);
    let settings = ManagedMusicVideoConfiguration {
        ffmpeg: Some(ffmpeg),
        ffprobe: Some(ffprobe),
        tools: options,
        ..ManagedMusicVideoConfiguration::default()
    };
    let native = build_source(&settings, &tool, None).unwrap();
    native.validate().unwrap();
    // Exercise the existing assembly admission guard: the legacy128GiB
    // output default alone used768GiB and made every256GiB managed preset fail.
    assert!(
        source_assembly_working_bytes(&native, 1024, 4096, 2048).unwrap()
            < settings.maximum_working_bytes
    );
    assert!(native.recipe.interpolation_run.is_none());
    let bytes = serde_json::to_vec(&native).unwrap();
    let roundtrip =
        aniflow::music_video_recipe::SourceMusicVideoConfiguration::from_json_slice(&bytes)
            .unwrap();
    assert_eq!(native, roundtrip);
    let aniflow::music_video_recipe::MusicVideoUpscale::Upscayl { configuration } =
        native.recipe.upscale
    else {
        panic!("selected Upscayl")
    };
    assert_eq!(configuration.upscayl.sha256, tool.executable_sha256);
    assert_eq!(configuration.model.param.sha256, tool.param_sha256);
    assert!(configuration.managed_runtime.is_some());
}
#[test]
fn cache_and_run_confinement_checks_nonexistent_and_symlinked_descendants() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let options = options(root);
    assert!(
        guard_cache_separation(
            &options,
            &[root.join("managed/versions/pkg/license/output")]
        )
        .is_err()
    );
    assert!(guard_cache_separation(&options, &[root.to_path_buf()]).is_err());
    assert!(guard_cache_separation(&options, &[root.join("runs")]).is_ok());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("managed"), root.join("alias")).unwrap();
        fs::create_dir(root.join("managed")).unwrap();
        assert!(guard_cache_separation(&options, &[root.join("alias/runs")]).is_err());
    }
}
#[test]
fn active_selection_is_honored_and_corruption_does_not_fall_back() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let (mut catalog, downloads) = fixture();
    let mut second = catalog.packages[0].clone();
    second.id = "upscayl-fixture-alternate".to_owned();
    second.version = "2.0.0".to_owned();
    catalog.packages.push(second);
    let options = options(root);
    let store = ManagedStore::new(root.join("managed")).unwrap();
    store
        .acquire(
            &catalog,
            "upscayl-fixture-alternate",
            &downloads,
            &ManagedControl::default(),
        )
        .unwrap();
    store
        .activate(&catalog, "upscayl-fixture-alternate")
        .unwrap();
    assert_eq!(
        plan_tool(&catalog, &options, ManagedProvider::Upscayl)
            .unwrap()
            .package_id,
        "upscayl-fixture-alternate"
    );
    fs::write(
        store
            .package_root("upscayl-fixture-alternate")
            .unwrap()
            .join("upscayl-bin"),
        b"corrupt",
    )
    .unwrap();
    let plan = plan_tool(&catalog, &options, ManagedProvider::Upscayl).unwrap();
    assert_eq!(plan.package_id, "upscayl-fixture-alternate");
    assert!(!plan.ready);
    let mut explicit = options;
    explicit.upscayl.version = Some("1.0.0".to_owned());
    assert_eq!(
        plan_tool(&catalog, &explicit, ManagedProvider::Upscayl)
            .unwrap()
            .package_id,
        "upscayl-fixture-anime"
    );
}

#[test]
fn closed_managed_schemas_refuse_unknown_fields_and_invalid_policy_before_download() {
    let mut value = serde_json::to_value(ManagedMusicVideoConfiguration::default()).unwrap();
    value["unknown_download_source"] = serde_json::json!("https://example.com");
    assert!(serde_json::from_value::<ManagedMusicVideoConfiguration>(value).is_err());
    let settings = ManagedMusicVideoConfiguration {
        maximum_source_bytes: 0,
        ..ManagedMusicVideoConfiguration::default()
    };
    assert!(settings.validate().is_err());
    let temporary = tempfile::tempdir().unwrap();
    let (catalog, downloads) = fixture();
    let mut options = options(temporary.path());
    let manual = temporary.path().join("manual");
    write_exe(&manual);
    options.upscayl.executable = Some(manual);
    options.upscayl.version = Some("invalid / version".to_owned());
    assert!(
        resolve_tool(
            &catalog,
            &options,
            ManagedProvider::Upscayl,
            &downloads,
            &ManagedControl::default()
        )
        .is_err()
    );
    assert_eq!(downloads.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn relative_dot_cache_plan_normalizes_raw_store_and_role_paths_without_writes() {
    let temporary = tempfile::tempdir().unwrap();
    let unique = format!(
        "{}.planned-managed-cache",
        temporary.path().file_name().unwrap().to_string_lossy()
    );
    let relative = std::path::PathBuf::from(format!("./{unique}"));
    let expected = std::env::current_dir().unwrap().join(&unique);
    assert!(!expected.exists());
    let (catalog, downloads) = fixture();
    let options = ManagedOptions {
        cache_directory: Some(relative),
        ..ManagedOptions::default()
    };
    let report = plan_experience(&catalog, &options, true, false).unwrap();
    // Compare actual path bytes, not Path equality, which normalizes `.`.
    assert_eq!(report.cache_directory.as_os_str(), expected.as_os_str());
    let store = ManagedStore::new(expected.clone()).unwrap();
    let package_root = store.package_root("upscayl-fixture-anime").unwrap();
    for (role, relative_path) in [
        ("executable", "upscayl-bin"),
        ("model_parameter", "models/anime.param"),
        ("model_weights", "models/anime.bin"),
    ] {
        let planned = report.tools[0]
            .roles
            .iter()
            .find(|value| value.role == role)
            .unwrap();
        assert_eq!(
            planned.path.as_os_str(),
            package_root.join(relative_path).as_os_str()
        );
    }
    assert!(!expected.exists());
    assert_eq!(downloads.calls.load(Ordering::SeqCst), 0);
    let traversal = ManagedOptions {
        cache_directory: Some(format!("./../{unique}").into()),
        ..ManagedOptions::default()
    };
    assert!(plan_experience(&catalog, &traversal, true, false).is_err());
    assert!(!expected.exists());
}
