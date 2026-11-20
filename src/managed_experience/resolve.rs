use super::*;
use crate::toolchain::managed::{
    ManagedCatalog, ManagedControl, ManagedDownloader, ManagedFileRole, ManagedPackage,
    ManagedPlan, ManagedPlanState, ManagedPlatform, ManagedProvider, ManagedStore,
};
use crate::{Error, ErrorCategory, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRolePlan {
    pub role: String,
    pub path: PathBuf,
    pub origin: String,
    pub ready: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedToolPlan {
    pub provider: ManagedProvider,
    pub platform: ManagedPlatform,
    pub package_id: String,
    pub model: String,
    pub backend: String,
    pub ready: bool,
    pub acquisition: Option<ManagedPlan>,
    pub roles: Vec<ManagedRolePlan>,
    pub notes: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedExperiencePlan {
    pub schema: String,
    pub cache_directory: PathBuf,
    pub offline: bool,
    pub no_download: bool,
    pub ready: bool,
    pub tools: Vec<ManagedToolPlan>,
    pub next_action: String,
    pub native_qualification: String,
}

pub fn load_user_configuration(path: impl AsRef<Path>) -> Result<ManagedOptions> {
    let metadata = std::fs::symlink_metadata(path.as_ref())
        .map_err(|_| dependency("cannot inspect explicitly selected managed user configuration"))?;
    if !metadata.is_file() || metadata.len() > 65_536 {
        return Err(configuration(
            "managed user configuration must be a regular file of at most64KiB",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path.as_ref())
        .map_err(|_| dependency("cannot read explicitly selected managed user configuration"))?
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| dependency("cannot read explicitly selected managed user configuration"))?;
    if bytes.len() > 65_536 {
        return Err(configuration("managed user configuration exceeds 64 KiB"));
    }
    let value: ManagedUserConfiguration = serde_json::from_slice(&bytes)
        .map_err(|_| configuration("invalid closed managed user configuration"))?;
    if value.schema != MANAGED_OPTIONS_SCHEMA {
        return Err(configuration("unsupported managed options schema"));
    }
    Ok(value.options)
}
pub fn default_cache_directory() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ANIFLOW_MANAGED_CACHE") {
        return absolute(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| configuration("set --managed-cache-directory when HOME is unavailable"))?;
    let base = if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Caches/aniflow/managed")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join("AppData/Local"))
            .join("aniflow/managed")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".cache"))
            .join("aniflow/managed")
    };
    absolute(base)
}
fn root(options: &ManagedOptions) -> Result<PathBuf> {
    options
        .cache_directory
        .clone()
        .map(absolute)
        .transpose()?
        .map_or_else(default_cache_directory, Ok)
}
fn selected<'a>(
    catalog: &'a ManagedCatalog,
    provider: ManagedProvider,
    options: &ManagedToolOptions,
    store: &ManagedStore,
) -> Result<&'a ManagedPackage> {
    let platform = ManagedPlatform::current();
    if options.version.is_none() {
        let references = store.references()?;
        let active = references
            .active
            .iter()
            .map(|id| catalog.package(id))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|package| {
                package.provider == provider
                    && package.platform == platform
                    && options
                        .model
                        .as_ref()
                        .is_none_or(|model| &package.model_id == model)
            })
            .collect::<Vec<_>>();
        if active.len() > 1 {
            return Err(configuration(
                "multiple managed models are active; select --*-model explicitly",
            ));
        }
        if let Some(package) = active.first() {
            return Ok(package);
        }
    }
    let mut candidates = catalog.packages.iter().filter(|package| {
        package.provider == provider
            && package.platform == platform
            && (options.executable.is_some()
                || options
                    .version
                    .as_ref()
                    .is_none_or(|version| &package.version == version))
            && (has_explicit_models(options)
                || options
                    .model
                    .as_ref()
                    .is_none_or(|model| &package.model_id == model))
    });
    candidates.next().ok_or_else(|| dependency(format!("no managed {provider:?} package for {}/{} with the selected version/model; use explicit compatible paths or inspect the bundled support catalog", platform.os, platform.arch)))
}
fn role_paths(
    package: &ManagedPackage,
    options: &ManagedToolOptions,
    installed_root: &Path,
) -> Result<Vec<ManagedRolePlan>> {
    let role_file = |role| {
        package
            .files()
            .find(|file| file.role == role)
            .ok_or_else(|| configuration("managed catalog is missing a required role"))
    };
    let default_param = role_file(ManagedFileRole::ModelParameter)?;
    let default_bin = role_file(ManagedFileRole::ModelWeights)?;
    let name = options.model.as_deref().unwrap_or(&package.model_id);
    let stem = if name == "realesr-animevideov3" {
        format!("{name}-x4")
    } else {
        name.to_owned()
    };
    let model_param = options.model_param.clone().or_else(|| {
        options.model_directory.as_ref().map(|dir| {
            dir.join(if package.provider == ManagedProvider::Rife {
                "flownet.param".to_owned()
            } else {
                format!("{stem}.param")
            })
        })
    });
    let model_weights = options.model_weights.clone().or_else(|| {
        options.model_directory.as_ref().map(|dir| {
            dir.join(if package.provider == ManagedProvider::Rife {
                "flownet.bin".to_owned()
            } else {
                format!("{stem}.bin")
            })
        })
    });
    [
        (
            "executable",
            options.executable.clone(),
            installed_root.join(&package.executable),
        ),
        (
            "model_parameter",
            model_param,
            installed_root.join(&default_param.path),
        ),
        (
            "model_weights",
            model_weights,
            installed_root.join(&default_bin.path),
        ),
    ]
    .into_iter()
    .map(|(role, explicit, managed_path)| {
        let is_explicit = explicit.is_some();
        let path = absolute(explicit.unwrap_or(managed_path))?;
        if is_explicit {
            hash_file(&path, role == "executable")?;
        }
        Ok(ManagedRolePlan {
            role: role.to_owned(),
            ready: is_explicit,
            path,
            origin: if is_explicit { "explicit" } else { "managed" }.to_owned(),
        })
    })
    .collect()
}

/// Read-only: no directory creation, lock, network, subprocess or inference.
pub fn plan_tool(
    catalog: &ManagedCatalog,
    options: &ManagedOptions,
    provider: ManagedProvider,
) -> Result<ManagedToolPlan> {
    let tool = match provider {
        ManagedProvider::Upscayl => &options.upscayl,
        ManagedProvider::Rife => &options.rife,
    };
    validate_tool_declarations(provider, tool)?;
    if fully_manual(tool) {
        return manual_plan(provider, tool);
    }
    if tool.executable.is_some() && tool.version.is_none() {
        return Err(configuration(
            "an explicit executable requires its declared --*-version or config version",
        ));
    }
    let store = ManagedStore::new(root(options)?)?;
    let package = selected(catalog, provider, tool, &store)?;
    if tool.executable.is_none() {
        check_host_compatibility(package)?;
    }
    // A store plan supplies canonical target paths through an inspected receipt
    // when installed; otherwise package_id is a bounded store-owned path.
    let acquisition = store.plan(catalog, &package.id, options.offline || options.no_download)?;
    let installed = store.inspect(catalog, &package.id).ok();
    let install_root = installed
        .as_ref()
        .map(|value| value.root.clone())
        .map_or_else(|| store.package_root(&package.id), Ok)?;
    let mut roles = role_paths(package, tool, &install_root)?;
    let needs_managed = roles.iter().any(|role| role.origin == "managed");
    if matches!(acquisition.state, ManagedPlanState::Ready) {
        for role in &mut roles {
            if role.origin == "managed" {
                role.ready = true;
            }
        }
    }
    let ready = roles.iter().all(|role| role.ready);
    Ok(ManagedToolPlan {
        provider,
        platform: package.platform.clone(),
        package_id: package.id.clone(),
        model: tool
            .model
            .clone()
            .unwrap_or_else(|| package.model_id.clone()),
        backend: package.backend.clone(),
        ready,
        acquisition: needs_managed.then_some(acquisition),
        roles,
        notes: package.notes.clone(),
    })
}
pub fn plan_experience(
    catalog: &ManagedCatalog,
    options: &ManagedOptions,
    upscayl: bool,
    rife: bool,
) -> Result<ManagedExperiencePlan> {
    let mut tools = Vec::new();
    if upscayl {
        tools.push(plan_tool(catalog, options, ManagedProvider::Upscayl)?);
    }
    if rife {
        tools.push(plan_tool(catalog, options, ManagedProvider::Rife)?);
    }
    let ready = tools.iter().all(|tool| tool.ready);
    Ok(ManagedExperiencePlan { schema: MANAGED_EXPERIENCE_PLAN_SCHEMA.to_owned(), cache_directory: root(options)?, offline: options.offline, no_download: options.no_download, ready, tools,
        next_action: if ready { "Selected bytes are available. Actual native admission/qualification still runs in the existing workflow." } else if options.offline || options.no_download { "Use a verified installed/cached package or provide compatible explicit paths. No network is permitted." } else { "The selected actual run will acquire missing verified managed packages, then freeze exact native pins." }.to_owned(),
        native_qualification: "unestablished; artifact verification is not GPU, media or platform qualification".to_owned() })
}

/// Acquire only selected managed roles, then hash every effective local byte.
/// Explicit paths are checked before any installation and are never repaired.
pub fn resolve_tool(
    catalog: &ManagedCatalog,
    options: &ManagedOptions,
    provider: ManagedProvider,
    downloader: &dyn ManagedDownloader,
    control: &ManagedControl,
) -> Result<ResolvedManagedTool> {
    let plan = plan_tool(catalog, options, provider)?;
    if plan.package_id.is_empty() {
        return resolve_manual(
            provider,
            match provider {
                ManagedProvider::Upscayl => &options.upscayl,
                ManagedProvider::Rife => &options.rife,
            },
        );
    }
    let package = catalog.package(&plan.package_id)?;
    let store = ManagedStore::new(root(options)?)?;
    let mut control = control.clone();
    control.offline |= options.offline || options.no_download;
    let installed = if plan.acquisition.is_some() {
        Some(if plan.ready {
            store.inspect(catalog, &package.id)?
        } else {
            store.acquire(catalog, &package.id, downloader, &control)?
        })
    } else {
        None
    };
    let install_root = installed
        .as_ref()
        .map(|value| value.root.clone())
        .map_or_else(|| store.package_root(&package.id), Ok)?;
    let tool = match provider {
        ManagedProvider::Upscayl => &options.upscayl,
        ManagedProvider::Rife => &options.rife,
    };
    let roles = role_paths(package, tool, &install_root)?;
    let path = |role: &str| {
        roles
            .iter()
            .find(|value| value.role == role)
            .map(|value| value.path.clone())
            .ok_or_else(|| configuration("missing resolved managed role"))
    };
    let executable = path("executable")?;
    let param_path = path("model_parameter")?;
    let weights_path = path("model_weights")?;
    let (executable_sha256, _) = hash_file(&executable, true)?;
    let (param_sha256, param_bytes) = hash_file(&param_path, false)?;
    let (weights_sha256, weights_bytes) = hash_file(&weights_path, false)?;
    let model_directory = param_path
        .parent()
        .ok_or_else(|| configuration("model parameter has no parent directory"))?
        .to_path_buf();
    if provider == ManagedProvider::Rife && weights_path.parent() != Some(model_directory.as_path())
    {
        return Err(configuration(
            "RIFE parameter and weights must share their rife-v4.6 directory",
        ));
    }
    let runtime_files = if tool.executable.is_none() {
        installed
            .as_ref()
            .map(|value| {
                value
                    .receipt
                    .package
                    .files()
                    .filter(|file| file.role == ManagedFileRole::Runtime)
                    .map(|file| ResolvedManagedRuntimeFile {
                        path: value.root.join(&file.path),
                        relative_path: PathBuf::from(&file.path),
                        sha256: file.sha256.clone(),
                        byte_count: file.length,
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let installation_roots = installed
        .as_ref()
        .map(|value| {
            vec![
                root(options).unwrap_or_else(|_| value.root.clone()),
                value.root.clone(),
            ]
        })
        .unwrap_or_default();
    let identity = installed.map(|value| value.receipt.identity());
    Ok(ResolvedManagedTool {
        executable,
        version: tool
            .version
            .clone()
            .unwrap_or_else(|| package.version.clone()),
        model_id: tool
            .model
            .clone()
            .unwrap_or_else(|| package.model_id.clone()),
        model_scale: package.model_scale,
        model_directory,
        param_path,
        weights_path,
        executable_sha256,
        param_sha256,
        weights_sha256,
        param_bytes,
        weights_bytes,
        runtime_files,
        installation_roots,
        package_id: identity.as_ref().map(|value| value.package_id.clone()),
        package_sha256: identity.as_ref().map(|value| value.package_sha256.clone()),
        catalog_revision: identity.map(|value| value.catalog_revision),
    })
}

pub(crate) fn hash_file(path: &Path, executable: bool) -> Result<(String, u64)> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| {
        dependency(format!(
            "explicit or resolved file is unavailable: {}",
            path.display()
        ))
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 536_870_912 {
        return Err(dependency(format!(
            "expected a nonempty regular file of at most 512 MiB: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if executable && metadata.permissions().mode() & 0o111 == 0 {
            return Err(dependency(format!(
                "selected executable is not executable: {}",
                path.display()
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = executable;
    let mut file =
        File::open(path).map_err(|_| dependency("cannot open selected tool/model file"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    let mut observed = 0;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| dependency("cannot hash selected tool/model file"))?;
        if count == 0 {
            break;
        }
        observed += count as u64;
        if observed > 536_870_912 {
            return Err(dependency("selected file grew beyond its bound"));
        }
        digest.update(&buffer[..count]);
    }
    if observed != metadata.len() {
        return Err(dependency("selected file changed while hashing"));
    }
    Ok((format!("{:x}", digest.finalize()), observed))
}
pub(crate) fn absolute(path: PathBuf) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|_| configuration("cannot resolve current directory"))?
            .join(path)
    };
    if path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(configuration("managed paths must not contain traversal"));
    }
    // Components remove lexical `.` segments without resolving symlinks. Keep
    // the normalized bytes too: native pin validation checks textual segments,
    // whereas Path equality alone can consider `/root/./cache` equivalent.
    Ok(path.components().collect())
}
pub(crate) fn configuration(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}
pub(crate) fn dependency(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Dependency, message)
}

fn has_explicit_models(options: &ManagedToolOptions) -> bool {
    options.model_directory.is_some()
        || (options.model_param.is_some() && options.model_weights.is_some())
}
fn fully_manual(options: &ManagedToolOptions) -> bool {
    options.executable.is_some() && has_explicit_models(options)
}
fn manual_paths(
    provider: ManagedProvider,
    options: &ManagedToolOptions,
) -> Result<(PathBuf, PathBuf, PathBuf, String)> {
    if options
        .version
        .as_ref()
        .is_none_or(|value| value.is_empty())
    {
        return Err(configuration(
            "an explicit executable requires its declared --*-version or config version",
        ));
    }
    let model = options.model.clone().unwrap_or_else(|| {
        match provider {
            ManagedProvider::Upscayl => "realesr-animevideov3",
            ManagedProvider::Rife => "rife-v4.6",
        }
        .to_owned()
    });
    let stem = if model == "realesr-animevideov3" {
        format!("{model}-x4")
    } else {
        model.clone()
    };
    let param = options
        .model_param
        .clone()
        .or_else(|| {
            options.model_directory.as_ref().map(|dir| {
                dir.join(if provider == ManagedProvider::Rife {
                    "flownet.param".to_owned()
                } else {
                    format!("{stem}.param")
                })
            })
        })
        .ok_or_else(|| configuration("missing explicit model parameter"))?;
    let weights = options
        .model_weights
        .clone()
        .or_else(|| {
            options.model_directory.as_ref().map(|dir| {
                dir.join(if provider == ManagedProvider::Rife {
                    "flownet.bin".to_owned()
                } else {
                    format!("{stem}.bin")
                })
            })
        })
        .ok_or_else(|| configuration("missing explicit model weights"))?;
    Ok((
        absolute(
            options
                .executable
                .clone()
                .ok_or_else(|| configuration("missing explicit executable"))?,
        )?,
        absolute(param)?,
        absolute(weights)?,
        model,
    ))
}
fn manual_plan(provider: ManagedProvider, options: &ManagedToolOptions) -> Result<ManagedToolPlan> {
    let (executable, param, weights, model) = manual_paths(provider, options)?;
    let roles = [
        ("executable", executable),
        ("model_parameter", param),
        ("model_weights", weights),
    ]
    .into_iter()
    .map(|(role, path)| {
        hash_file(&path, role == "executable")?;
        Ok(ManagedRolePlan {
            role: role.to_owned(),
            path,
            origin: "explicit".to_owned(),
            ready: true,
        })
    })
    .collect::<Result<Vec<_>>>()?;
    Ok(ManagedToolPlan{provider,platform:ManagedPlatform::current(),package_id:String::new(),model,backend:"explicit compatible local provider".to_owned(),ready:true,acquisition:None,roles,notes:vec!["Manual bytes are hashed locally; no managed receipt or native qualification is claimed.".to_owned()]})
}
fn resolve_manual(
    provider: ManagedProvider,
    options: &ManagedToolOptions,
) -> Result<ResolvedManagedTool> {
    let (executable, param_path, weights_path, model_id) = manual_paths(provider, options)?;
    let (executable_sha256, _) = hash_file(&executable, true)?;
    let (param_sha256, param_bytes) = hash_file(&param_path, false)?;
    let (weights_sha256, weights_bytes) = hash_file(&weights_path, false)?;
    let model_directory = param_path
        .parent()
        .ok_or_else(|| configuration("model path has no parent"))?
        .to_path_buf();
    if provider == ManagedProvider::Rife && weights_path.parent() != Some(model_directory.as_path())
    {
        return Err(configuration(
            "RIFE parameter and weights must share their rife-v4.6 directory",
        ));
    }
    Ok(ResolvedManagedTool {
        executable,
        version: options.version.clone().unwrap_or_default(),
        model_id,
        model_scale: if provider == ManagedProvider::Rife {
            2
        } else {
            4
        },
        model_directory,
        param_path,
        weights_path,
        executable_sha256,
        param_sha256,
        weights_sha256,
        param_bytes,
        weights_bytes,
        runtime_files: Vec::new(),
        installation_roots: Vec::new(),
        package_id: None,
        package_sha256: None,
        catalog_revision: None,
    })
}

/// Use the exact same per-selection effective options as actual execution.
pub fn plan_source_dependencies(
    catalog: &ManagedCatalog,
    configuration: &ManagedMusicVideoConfiguration,
) -> Result<ManagedExperiencePlan> {
    let mut report = plan_experience(catalog, &configuration.tools, true, false)?;
    if let Some(settings) = &configuration.interpolation {
        report
            .tools
            .push(plan_tool(catalog, &settings.tools, ManagedProvider::Rife)?);
    }
    report.ready = report.tools.iter().all(|tool| tool.ready);
    if !report.ready {
        report.next_action="The selected actual run acquires only missing selected packages when network is allowed; offline/no-download requires verified cached or installed bytes.".to_owned();
    }
    Ok(report)
}

/// Keep generated/owned cache trees disjoint from source, config and run roots.
/// Missing descendants are normalized lexically; existing ancestors are resolved
/// so a symlink cannot turn an apparently separate output into the package tree.
pub fn guard_cache_separation(options: &ManagedOptions, paths: &[PathBuf]) -> Result<()> {
    let cache = resolved_ancestor(&root(options)?)?;
    for path in paths {
        let value = resolved_ancestor(&absolute(path.clone())?)?;
        if value.starts_with(&cache) || cache.starts_with(&value) {
            return Err(configuration(format!(
                "managed cache and source/config/run paths must be disjoint: {} overlaps {}",
                cache.display(),
                value.display()
            )));
        }
    }
    Ok(())
}
fn resolved_ancestor(path: &Path) -> Result<PathBuf> {
    let mut ancestor = path;
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .ok_or_else(|| configuration("cannot resolve managed path ancestor"))?
                .to_os_string(),
        );
        ancestor = ancestor
            .parent()
            .ok_or_else(|| configuration("cannot resolve managed path ancestor"))?;
    }
    let mut result = ancestor
        .canonicalize()
        .map_err(|_| configuration("cannot resolve managed path ancestor"))?;
    for name in suffix.into_iter().rev() {
        result.push(name);
    }
    Ok(result)
}

/// Conservative crash-safe retention before native consumption. The reference
/// identifies exact native configuration, not an accepted or completed run.
/// It remains until an explicit lifecycle release after all matching runs have
/// been removed or exported; failures never release resumable dependencies.
pub fn retain_native_configuration(
    options: &ManagedOptions,
    tool: &ResolvedManagedTool,
    configuration_bytes: &[u8],
) -> Result<Option<String>> {
    let Some(id) = &tool.package_id else {
        return Ok(None);
    };
    let reference = format!("configuration-{:x}", Sha256::digest(configuration_bytes));
    ManagedStore::new(root(options)?)?.retain(id, &reference)?;
    Ok(Some(reference))
}

fn check_host_compatibility(package: &ManagedPackage) -> Result<()> {
    if package.platform.os != "macos" {
        return Ok(());
    }
    let bytes = std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
        .map_err(|_| {
            dependency("cannot observe macOS ProductVersion before managed acquisition")
        })?;
    let document = roxmltree::Document::parse(&bytes)
        .map_err(|_| dependency("cannot parse macOS system version"))?;
    let version = document
        .descendants()
        .find(|node| node.has_tag_name("key") && node.text() == Some("ProductVersion"))
        .and_then(|node| node.next_sibling_element())
        .and_then(|node| node.text())
        .ok_or_else(|| dependency("macOS ProductVersion is absent"))?;
    let major = version
        .split('.')
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| dependency("invalid observed macOS version"))?;
    let minimum = match package.provider {
        ManagedProvider::Upscayl => {
            if package.platform.arch == "aarch64" {
                14
            } else {
                13
            }
        }
        ManagedProvider::Rife => 11,
    };
    if major < minimum {
        return Err(dependency(format!(
            "selected managed {:?} requires macOS{minimum}+ on{}; observed{version}; provide an explicitly compatible tool instead",
            package.provider, package.platform.arch
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSourceDependencyPlan {
    pub schema: String,
    pub source_video: PathBuf,
    pub source_sha256: String,
    pub source_bytes: u64,
    pub source_media_probe_performed: bool,
    pub output_geometry_pending: bool,
    pub requested: ManagedMusicVideoConfiguration,
    pub dependencies: ManagedExperiencePlan,
    pub ordered_effects: Vec<String>,
}
/// Exact file identity and explicit finite requested policy, without pretending
/// source geometry, decode support, free disk or native output were observed.
pub fn plan_source_preview(
    source: &Path,
    configuration: &ManagedMusicVideoConfiguration,
    catalog: &ManagedCatalog,
) -> Result<ManagedSourceDependencyPlan> {
    configuration.validate()?;
    let source_video = absolute(source.to_path_buf())?;
    let (source_sha256, source_bytes) = crate::frame_io::identity(&source_video, 68_719_476_736)?;
    let dependencies = plan_source_dependencies(catalog, configuration)?;
    let mut ordered_effects = vec![
        "acquire only selected missing verified managed dependencies when network is allowed"
            .to_owned(),
        "extract source-bound RGB PNGs into a fresh run".to_owned(),
        "import exact indexed frames".to_owned(),
        format!(
            "Upscayl spatial processing at requested {}x, Vulkan device0, bounded {}-frame checkpoints",
            configuration.output_scale, configuration.frames_per_chunk
        ),
    ];
    if configuration.interpolation.is_some() {
        ordered_effects.push("explicitly selected serial RIFE midpoint interpolation".to_owned());
    }
    ordered_effects.push(
        "assemble lossless master and H.264 delivery with the selected original audio".to_owned(),
    );
    Ok(ManagedSourceDependencyPlan {
        schema: "aniflow.managed-source-plan/v1".to_owned(),
        source_video,
        source_sha256,
        source_bytes,
        source_media_probe_performed: false,
        output_geometry_pending: true,
        requested: configuration.clone(),
        dependencies,
        ordered_effects,
    })
}

fn validate_tool_declarations(
    provider: ManagedProvider,
    options: &ManagedToolOptions,
) -> Result<()> {
    if let Some(version) = &options.version {
        if version.is_empty()
            || version.len() > 128
            || !version
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || b".-+".contains(&value))
        {
            return Err(configuration(
                "tool release/version must be a bounded portable token",
            ));
        }
        if options.executable.is_some() && semver::Version::parse(version).is_err() {
            return Err(configuration(
                "explicit manual executable version must be an exact semantic version (for example2.15.0); managed date releases are selected without an executable override",
            ));
        }
    }
    if let Some(model) = &options.model {
        if provider == ManagedProvider::Upscayl {
            crate::upscayl_recovery::validate_processing_policy(model, 4, 4, 32, 0, "1:1:1")?;
        } else if model != "rife-v4.6" {
            return Err(configuration(
                "the supported RIFE profile requires model rife-v4.6",
            ));
        }
    }
    Ok(())
}
