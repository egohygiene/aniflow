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
        output_file: PathBuf,
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
            DiagnosticsCommands::Bundle {
                request,
                output_file,
            } => serde_json::to_value(owner.create_bundle(
                &owner.read_request(&request, &cancel)?,
                &output_file,
                &cancel,
            )?),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_output_file_preserves_global_presentation() {
        let command = ["diagnostics", "bundle", "--request", "support.json"];
        for placement in 0..3 {
            let mut arguments = vec!["aniflow"];
            if placement == 0 {
                arguments.extend(["--output", "json"]);
            }
            arguments.extend(command);
            arguments.extend(["--output-file", "private/support.json"]);
            if placement == 1 {
                arguments.extend(["--output", "json"]);
            }
            let parsed = Cli::try_parse_from(arguments).expect("independent bundle destination");
            assert_eq!(matches!(parsed.output, OutputFormat::Json), placement != 2);
            let Commands::Diagnostics {
                command: DiagnosticsCommands::Bundle { output_file, .. },
            } = parsed.command
            else {
                panic!("expected diagnostics bundle");
            };
            assert_eq!(output_file, PathBuf::from("private/support.json"));
        }
        assert!(Cli::try_parse_from(std::iter::once("aniflow").chain(command)).is_err());
        assert!(
            Cli::try_parse_from(
                std::iter::once("aniflow")
                    .chain(command)
                    .chain(["--output", "private/support.json"])
            )
            .is_err()
        );
    }
}
