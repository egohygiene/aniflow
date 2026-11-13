//! Thin native-envelope delivery for the local support SDK.
use super::*;
#[derive(Debug, Subcommand)]
pub(super) enum DiagnosticsCommands {
    Preview {
        #[arg(long)]
        request: PathBuf,
    },
    Bundle {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Inspect {
        #[arg(long)]
        bundle: PathBuf,
    },
    Doctor,
}
impl DiagnosticsCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Preview { .. } => CommandName::DiagnosticsPreview,
            Self::Bundle { .. } => CommandName::DiagnosticsBundle,
            Self::Inspect { .. } => CommandName::DiagnosticsInspect,
            Self::Doctor => CommandName::DiagnosticsDoctor,
        }
    }
}
pub(super) fn dispatch(
    command: DiagnosticsCommands,
    presentation: Presentation,
) -> CommandResult<()> {
    let name = command.name();
    let owner = aniflow::diagnostics::owner();
    let cancel = || false;
    let result = (|| -> std::result::Result<Value, aniflow::diagnostics::Error> {
        match command {
            DiagnosticsCommands::Preview { request } => serde_json::to_value(
                owner.preview(&owner.read_request(&request, &cancel)?, &cancel)?,
            ),
            DiagnosticsCommands::Bundle { request, output } => serde_json::to_value(
                owner.create_bundle(&owner.read_request(&request, &cancel)?, &output, &cancel)?,
            ),
            DiagnosticsCommands::Inspect { bundle } => {
                serde_json::to_value(owner.inspect_bundle(&bundle, &cancel)?)
            }
            DiagnosticsCommands::Doctor => serde_json::to_value(owner.health()),
        }
        .map_err(|_| aniflow::diagnostics::Error::Invalid)
    })()
    .map_err(|error| {
        let category = match error {
            aniflow::diagnostics::Error::Unsupported => ErrorCategory::Dependency,
            aniflow::diagnostics::Error::Io => ErrorCategory::Io,
            _ => ErrorCategory::Input,
        };
        Error::new(category, error.to_string())
    })?;
    print_result(name, presentation, &result, || {
        println!(
            "{}",
            serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| "support result unavailable".into())
        )
    })
}
