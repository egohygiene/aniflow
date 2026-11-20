//! Presentation and argument delivery for the managed toolchain store.
use super::*;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aniflow::toolchain::managed::{
    HttpsDownloader, ManagedControl, ManagedPlan, ManagedPlatform, ManagedProgress, ManagedReceipt,
    ManagedReferences, ManagedStore, bundled_catalog,
};

#[derive(Debug, Args)]
pub(super) struct ManagedCommandArguments {
    /// Select the dedicated managed cache; defaults to the platform user cache.
    #[arg(long, global = true)]
    cache_directory: Option<PathBuf>,
    /// Use verified local packages or cached archives without network access.
    #[arg(long, global = true)]
    offline: bool,
    /// Refuse downloads; verified cached archives may still be installed.
    #[arg(long, global = true)]
    no_download: bool,
    /// Maximum HTTPS acquisition time per artifact, in seconds.
    #[arg(long, global = true, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..=3600))]
    timeout: u64,
    #[command(subcommand)]
    command: ManagedCommands,
}

#[derive(Debug, Subcommand)]
enum ManagedCommands {
    /// Verify readiness for one package, or all bundled current-platform packages.
    Doctor { package_id: Option<String> },
    /// Inspect exact missing download and installed byte costs without mutation.
    Plan { package_id: String },
    /// List verified installed packages and active/retained references.
    List,
    /// Acquire and verify one immutable package; activation is a separate action.
    Install { package_id: String },
    /// Quarantine and replace a selected corrupt, unreferenced package.
    Repair { package_id: String },
    /// Select an already installed verified package for its provider.
    Activate { package_id: String },
    /// Deselect a package without removing retained-run references or files.
    Deactivate { package_id: String },
    /// Install and activate the exact selected catalog package, keeping old versions.
    Upgrade { package_id: String },
    /// Activate an already installed package without downloading.
    Rollback { package_id: String },
    /// Remove only a verified inactive package with no retained-run references.
    Uninstall { package_id: String },
    /// Protect an installed package for one retained run.
    Retain {
        package_id: String,
        #[arg(long)]
        run_id: String,
    },
    /// Release one package/run reference; preserve the run and package files.
    Release {
        package_id: String,
        #[arg(long)]
        run_id: String,
    },
}

#[derive(Debug, Clone, Serialize)]
struct ManagedCommandReport {
    schema: &'static str,
    operation: &'static str,
    root: PathBuf,
    data: Value,
}

#[derive(Serialize)]
struct DoctorData {
    catalog_revision: String,
    platform: ManagedPlatform,
    packages: Vec<ManagedPlan>,
    references: ManagedReferences,
}

#[derive(Serialize)]
struct ListData {
    installed: Vec<ManagedReceipt>,
    references: ManagedReferences,
}

#[derive(Serialize)]
struct ReferenceData {
    package_id: String,
    references: ManagedReferences,
}

fn report_data(value: impl Serialize) -> Result<Value> {
    serde_json::to_value(value).map_err(|error| {
        Error::new(
            ErrorCategory::Internal,
            format!("cannot serialize managed command result: {error}"),
        )
    })
}

fn acquisition_control(offline: bool, timeout: u64) -> Result<ManagedControl> {
    // This is called exactly once in an acquisition branch. Read-only commands
    // never install a process-global signal handler.
    let cancellation = cli_cancellation_token()?;
    let last_update = Mutex::new(Instant::now());
    let observer = Arc::new(move |event: ManagedProgress| match event {
        ManagedProgress::DownloadStarted { artifact_id, bytes } => {
            eprintln!(
                "managed: downloading {} ({bytes} bytes)",
                escape_terminal_controls(&artifact_id)
            );
        }
        ManagedProgress::DownloadProgress {
            artifact_id,
            bytes,
            total,
        } => {
            if let Ok(mut last) = last_update.lock() {
                if bytes == total || last.elapsed() >= Duration::from_secs(1) {
                    eprintln!(
                        "managed: {} {bytes}/{total} bytes",
                        escape_terminal_controls(&artifact_id)
                    );
                    *last = Instant::now();
                }
            }
        }
        ManagedProgress::ArchiveVerified { artifact_id } => {
            eprintln!(
                "managed: verified archive {}",
                escape_terminal_controls(&artifact_id)
            );
        }
        ManagedProgress::FileInstalled { path, bytes } => {
            eprintln!(
                "managed: staged {} ({bytes} bytes)",
                escape_terminal_controls(&path)
            );
        }
        ManagedProgress::Installed { package_id } => {
            eprintln!(
                "managed: installed {}",
                escape_terminal_controls(&package_id)
            );
        }
    });
    Ok(ManagedControl {
        offline,
        cancellation,
        timeout_seconds: timeout,
        observer: Some(observer),
    })
}

pub(super) fn dispatch(
    arguments: ManagedCommandArguments,
    presentation: Presentation,
) -> CommandResult<()> {
    if !(1..=3600).contains(&arguments.timeout) {
        return Err(Error::new(
            ErrorCategory::Input,
            "managed download timeout must be 1..=3600 seconds",
        )
        .into());
    }
    let root = arguments
        .cache_directory
        .map_or_else(aniflow::managed_experience::default_cache_directory, Ok)?;
    // Constructing the store, reading its catalog and preparing read-only
    // reports do not create the cache, ownership marker or writer lock.
    let store = ManagedStore::new(root)?;
    let catalog = bundled_catalog()?;
    let offline = arguments.offline || arguments.no_download;
    let (operation, data) = match arguments.command {
        ManagedCommands::Doctor { package_id } => {
            let platform = ManagedPlatform::current();
            let packages = match package_id {
                Some(id) => vec![store.plan(&catalog, &id, offline)?],
                None => catalog
                    .packages
                    .iter()
                    .filter(|package| package.platform == platform)
                    .map(|package| store.plan(&catalog, &package.id, offline))
                    .collect::<Result<Vec<_>>>()?,
            };
            (
                "doctor",
                report_data(DoctorData {
                    catalog_revision: catalog.revision.clone(),
                    platform,
                    packages,
                    references: store.references()?,
                })?,
            )
        }
        ManagedCommands::Plan { package_id } => (
            "plan",
            report_data(store.plan(&catalog, &package_id, offline)?)?,
        ),
        ManagedCommands::List => (
            "list",
            report_data(ListData {
                installed: store.list()?,
                references: store.references()?,
            })?,
        ),
        ManagedCommands::Install { package_id } => {
            let control = acquisition_control(offline, arguments.timeout)?;
            (
                "install",
                report_data(store.acquire(&catalog, &package_id, &HttpsDownloader, &control)?)?,
            )
        }
        ManagedCommands::Repair { package_id } => {
            let control = acquisition_control(offline, arguments.timeout)?;
            (
                "repair",
                report_data(store.repair(&catalog, &package_id, &HttpsDownloader, &control)?)?,
            )
        }
        ManagedCommands::Activate { package_id } => (
            "activate",
            report_data(store.activate(&catalog, &package_id)?)?,
        ),
        ManagedCommands::Deactivate { package_id } => {
            store.deactivate(&package_id)?;
            (
                "deactivate",
                report_data(ReferenceData {
                    package_id,
                    references: store.references()?,
                })?,
            )
        }
        ManagedCommands::Upgrade { package_id } => {
            let control = acquisition_control(offline, arguments.timeout)?;
            (
                "upgrade",
                report_data(store.upgrade(&catalog, &package_id, &HttpsDownloader, &control)?)?,
            )
        }
        ManagedCommands::Rollback { package_id } => (
            "rollback",
            report_data(store.rollback(&catalog, &package_id)?)?,
        ),
        ManagedCommands::Uninstall { package_id } => {
            store.uninstall(&package_id)?;
            (
                "uninstall",
                report_data(ReferenceData {
                    package_id,
                    references: store.references()?,
                })?,
            )
        }
        ManagedCommands::Retain { package_id, run_id } => {
            store.retain(&package_id, &run_id)?;
            (
                "retain",
                report_data(ReferenceData {
                    package_id,
                    references: store.references()?,
                })?,
            )
        }
        ManagedCommands::Release { package_id, run_id } => {
            store.release_retention(&package_id, &run_id)?;
            (
                "release",
                report_data(ReferenceData {
                    package_id,
                    references: store.references()?,
                })?,
            )
        }
    };
    let report = ManagedCommandReport {
        schema: "aniflow.managed-command/v1",
        operation,
        root: store.root().to_owned(),
        data,
    };
    print_result(CommandName::ToolchainManaged, presentation, &report, || {
        print_human(&report);
    })
}

fn print_plan(plan: &Value) {
    let id = plan["package_id"].as_str().unwrap_or("unknown package");
    let state = plan["state"].as_str().unwrap_or("unknown");
    let bytes = plan["network_bytes"].as_u64().unwrap_or(0);
    println!(
        "  {}: {state}; missing download {bytes} bytes",
        escape_terminal_controls(id)
    );
    if let Some(message) = plan["message"].as_str() {
        println!("    {}", escape_terminal_controls(message));
    }
}

fn print_human(report: &ManagedCommandReport) {
    println!("managed toolchain {}", report.operation);
    println!(
        "  cache: {}",
        escape_terminal_controls(&report.root.display().to_string())
    );
    match report.operation {
        "doctor" => {
            if let Some(packages) = report.data["packages"].as_array() {
                for package in packages {
                    print_plan(package);
                }
                if packages.is_empty() {
                    println!("  No bundled package is available for this platform.");
                }
            }
        }
        "plan" => print_plan(&report.data),
        "list" => {
            if let Some(installed) = report.data["installed"].as_array() {
                for receipt in installed {
                    if let Some(id) = receipt["package"]["id"].as_str() {
                        println!("  {}", escape_terminal_controls(id));
                    }
                }
                if installed.is_empty() {
                    println!("  No packages installed.");
                }
            }
        }
        _ => {
            if let Some(id) = report.data["receipt"]["package"]["id"]
                .as_str()
                .or_else(|| report.data["package_id"].as_str())
            {
                println!("  {}", escape_terminal_controls(id));
            }
        }
    }
    if let Some(active) = report.data["references"]["active"].as_array() {
        for id in active.iter().filter_map(Value::as_str) {
            println!("  active: {}", escape_terminal_controls(id));
        }
    }
    if let Some(retained) = report.data["references"]["retained"].as_array() {
        for reference in retained {
            if let (Some(package), Some(run)) = (
                reference["package_id"].as_str(),
                reference["run_id"].as_str(),
            ) {
                println!(
                    "  retained: {} for run {}",
                    escape_terminal_controls(package),
                    escape_terminal_controls(run)
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Parser)]
    struct ManagedParser {
        #[command(flatten)]
        arguments: ManagedCommandArguments,
    }

    #[test]
    fn shared_flags_parse_before_and_after_managed_subcommands() {
        for arguments in [
            vec![
                "managed",
                "--cache-directory",
                "private/cache",
                "--offline",
                "--timeout",
                "12",
                "plan",
                "selected-package",
            ],
            vec![
                "managed",
                "plan",
                "selected-package",
                "--cache-directory",
                "private/cache",
                "--offline",
                "--timeout",
                "12",
            ],
        ] {
            let parsed = ManagedParser::try_parse_from(arguments).expect("shared option placement");
            assert!(parsed.arguments.offline);
            assert_eq!(parsed.arguments.timeout, 12);
            assert_eq!(
                parsed.arguments.cache_directory,
                Some(PathBuf::from("private/cache"))
            );
            assert!(
                matches!(parsed.arguments.command, ManagedCommands::Plan { package_id } if package_id == "selected-package")
            );
        }
    }

    #[test]
    fn acquisition_timeout_and_selected_retention_are_explicit() {
        for timeout in ["0", "3601"] {
            assert!(
                ManagedParser::try_parse_from([
                    "managed",
                    "install",
                    "selected-package",
                    "--timeout",
                    timeout
                ])
                .is_err()
            );
        }
        assert!(ManagedParser::try_parse_from(["managed", "retain", "selected-package"]).is_err());
        let parsed = ManagedParser::try_parse_from([
            "managed",
            "release",
            "selected-package",
            "--run-id",
            "run-1",
            "--no-download",
        ])
        .expect("selected reference");
        assert!(parsed.arguments.no_download);
        assert!(
            matches!(parsed.arguments.command, ManagedCommands::Release { package_id, run_id } if package_id == "selected-package" && run_id == "run-1")
        );
        let doctor = ManagedParser::try_parse_from(["managed", "doctor"]).expect("platform doctor");
        assert!(matches!(
            doctor.arguments.command,
            ManagedCommands::Doctor { package_id: None }
        ));
    }
}
