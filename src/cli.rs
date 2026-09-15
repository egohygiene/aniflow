use std::path::PathBuf;
use std::process::ExitCode;

use aniflow::{
    CommandName, DoctorReport, Error, ErrorCategory, HostResources, MachineEnvelope,
    MediaInspection, PipelineInputBinding, PipelinePlan, PipelinePlanningContext,
    PipelinePlanningFailure, PipelineV3Plan, ProgressState, ReconstructionReport, Result,
    RunOperation, RunOutcome, RunProgress, RunRequest, RunStatus, SegmentMode, SegmentOutcome,
    SegmentPlan, SegmentProgress, SegmentRequest, SideEffect,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(
    name = "aniflow",
    version,
    about = "Define the pipeline once. Transform every frame. Rebuild the experience."
)]
pub struct Cli {
    /// Select human presentation or the versioned JSON contract.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    output: OutputFormat,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SegmentModeArgument {
    StreamCopy,
    TranscodeH264Aac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SideEffectArgument {
    FilesystemRead,
    FilesystemWrite,
    EnvironmentRead,
    Subprocess,
    Network,
    Ai,
    Gpu,
    Publish,
}

impl From<SideEffectArgument> for SideEffect {
    fn from(value: SideEffectArgument) -> Self {
        match value {
            SideEffectArgument::FilesystemRead => Self::FilesystemRead,
            SideEffectArgument::FilesystemWrite => Self::FilesystemWrite,
            SideEffectArgument::EnvironmentRead => Self::EnvironmentRead,
            SideEffectArgument::Subprocess => Self::Subprocess,
            SideEffectArgument::Network => Self::Network,
            SideEffectArgument::Ai => Self::Ai,
            SideEffectArgument::Gpu => Self::Gpu,
            SideEffectArgument::Publish => Self::Publish,
        }
    }
}

fn parse_pipeline_input_binding(value: &str) -> std::result::Result<PipelineInputBinding, String> {
    let (artifact_id, path) = value
        .split_once('=')
        .ok_or_else(|| "expected ARTIFACT_ID=PATH".to_owned())?;
    if artifact_id.is_empty() {
        return Err("artifact id cannot be empty".to_owned());
    }
    if path.is_empty() {
        return Err("input path cannot be empty".to_owned());
    }
    Ok(PipelineInputBinding::new(artifact_id, path))
}

impl From<SegmentModeArgument> for SegmentMode {
    fn from(value: SegmentModeArgument) -> Self {
        match value {
            SegmentModeArgument::StreamCopy => Self::StreamCopy,
            SegmentModeArgument::TranscodeH264Aac => Self::TranscodeH264Aac,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presentation {
    Human,
    Machine,
    LegacyInspectJson,
}

type CommandResult<T> = std::result::Result<T, CommandFailure>;

#[derive(Debug)]
struct CommandFailure {
    error: Error,
    planning: Option<PipelinePlanningFailure>,
}

impl From<Error> for CommandFailure {
    fn from(error: Error) -> Self {
        Self {
            error,
            planning: None,
        }
    }
}

impl From<PipelinePlanningFailure> for CommandFailure {
    fn from(planning: PipelinePlanningFailure) -> Self {
        Self {
            error: Error::new(planning.category(), planning.message.clone()),
            planning: Some(planning),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Verify required runtime dependencies.
    Doctor {
        /// Also verify every enabled tool required by this pipeline.
        #[arg(long)]
        pipeline: Option<PathBuf>,
    },
    /// Inspect a source video with ffprobe.
    Inspect {
        /// Source video to inspect.
        input: PathBuf,
        /// Compatibility alias for `--output json`.
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Print the stages that would execute without modifying media.
    Plan {
        /// Source video to process.
        #[arg(long)]
        input: PathBuf,
        /// Pipeline YAML file.
        #[arg(long)]
        pipeline: PathBuf,
    },
    /// Resolve a Pipeline v3 configuration without executing providers or writing artifacts.
    PlanV3 {
        /// Pipeline v3 YAML file.
        #[arg(long)]
        pipeline: PathBuf,
        /// Bind one declared input artifact to a local file or directory.
        #[arg(
            long,
            value_name = "ARTIFACT_ID=PATH",
            value_parser = parse_pipeline_input_binding
        )]
        input: Vec<PipelineInputBinding>,
        /// Explicit portable provider registration document.
        #[arg(long)]
        provider_registration: Vec<PathBuf>,
        /// Observed logical CPU threads available to providers.
        #[arg(long)]
        host_cpu_threads: u16,
        /// Observed host memory available to providers, in MiB.
        #[arg(long)]
        host_memory_mib: u64,
        /// Observed host storage available to providers, in MiB.
        #[arg(long)]
        host_storage_mib: u64,
        /// Declare that a GPU is available on the host.
        #[arg(long)]
        host_gpu_available: bool,
        /// Declare that network access is available on the host.
        #[arg(long)]
        host_network_available: bool,
        /// Authorize one provider side effect during later execution.
        #[arg(long, value_enum)]
        allow_side_effect: Vec<SideEffectArgument>,
        /// Require resolution to reject network-dependent providers.
        #[arg(long)]
        offline: bool,
    },
    /// Start a new isolated pipeline run.
    Run {
        /// Source video to process.
        #[arg(long)]
        input: PathBuf,
        /// Pipeline YAML file.
        #[arg(long)]
        pipeline: PathBuf,
        /// Parent directory for the new run.
        #[arg(long = "output-directory", visible_alias = "output-dir")]
        output_directory: Option<PathBuf>,
    },
    /// Continue an interrupted or failed run.
    Resume {
        /// Existing aniflow run directory.
        run_directory: PathBuf,
    },
    /// Display stage and artifact status for a run.
    Status {
        /// Existing aniflow run directory.
        run_directory: PathBuf,
    },
    /// Plan, execute, resume, or reconstruct a short-segment workflow.
    Segment {
        #[command(subcommand)]
        command: SegmentCommands,
    },
}

#[derive(Debug, Subcommand)]
enum SegmentCommands {
    /// Inspect and normalize a segmentation request without writing artifacts.
    Plan {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        segment_duration_ms: u64,
        #[arg(long, value_enum, default_value_t = SegmentModeArgument::StreamCopy)]
        mode: SegmentModeArgument,
        #[arg(long, default_value_t = 3_600)]
        process_timeout_seconds: u64,
    },
    /// Start a resumable segmentation run.
    Run {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        segment_duration_ms: u64,
        #[arg(long, value_enum, default_value_t = SegmentModeArgument::StreamCopy)]
        mode: SegmentModeArgument,
        #[arg(long, default_value_t = 3_600)]
        process_timeout_seconds: u64,
    },
    /// Resume an interrupted segmentation run from its verified prefix.
    Resume {
        #[arg(long)]
        run_directory: PathBuf,
    },
    /// Reconstruct and validate a complete segment manifest.
    Reconstruct {
        #[arg(long)]
        run_directory: PathBuf,
        /// Reconstructed media destination.
        #[arg(long)]
        output_file: PathBuf,
    },
}

impl Commands {
    const fn name(&self) -> CommandName {
        match self {
            Self::Doctor { .. } => CommandName::Doctor,
            Self::Inspect { .. } => CommandName::Inspect,
            Self::Plan { .. } => CommandName::Plan,
            Self::PlanV3 { .. } => CommandName::PlanV3,
            Self::Run { .. } => CommandName::Run,
            Self::Resume { .. } => CommandName::Resume,
            Self::Status { .. } => CommandName::Status,
            Self::Segment { command } => match command {
                SegmentCommands::Plan { .. } => CommandName::SegmentPlan,
                SegmentCommands::Run { .. } => CommandName::SegmentRun,
                SegmentCommands::Resume { .. } => CommandName::SegmentResume,
                SegmentCommands::Reconstruct { .. } => CommandName::SegmentReconstruct,
            },
        }
    }

    const fn requests_legacy_json(&self) -> bool {
        matches!(self, Self::Inspect { json: true, .. })
    }
}

pub fn execute() -> ExitCode {
    let cli = Cli::parse();
    let command = cli.command.name();
    let presentation = match (cli.output, cli.command.requests_legacy_json()) {
        (OutputFormat::Json, _) => Presentation::Machine,
        (OutputFormat::Human, true) => Presentation::LegacyInspectJson,
        (OutputFormat::Human, false) => Presentation::Human,
    };

    match dispatch(cli.command, presentation) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            if let Err(render_error) = print_error(command, presentation, &failure) {
                eprintln!("error: {render_error}");
                return ExitCode::from(ErrorCategory::Internal.exit_code());
            }
            ExitCode::from(failure.error.category().exit_code())
        }
    }
}

fn dispatch(command: Commands, presentation: Presentation) -> CommandResult<()> {
    match command {
        Commands::Doctor { pipeline } => {
            let report = aniflow::doctor(pipeline.as_deref())?;
            if !report.is_ready() {
                if presentation == Presentation::Human {
                    print_doctor(&report);
                }
                return Err(Error::new(
                    ErrorCategory::Dependency,
                    format!(
                        "install the missing runtime dependencies: {}",
                        report.missing_commands().collect::<Vec<_>>().join(", ")
                    ),
                )
                .into());
            }
            print_result(CommandName::Doctor, presentation, &report, || {
                print_doctor(&report);
            })
        }
        Commands::Inspect { input, .. } => {
            let inspection = aniflow::inspect(input)?;
            print_result(CommandName::Inspect, presentation, &inspection, || {
                print_inspection(&inspection);
            })
        }
        Commands::Plan { input, pipeline } => {
            let plan = aniflow::plan(input, pipeline)?;
            print_result(CommandName::Plan, presentation, &plan, || print_plan(&plan))
        }
        Commands::PlanV3 {
            pipeline,
            input,
            provider_registration,
            host_cpu_threads,
            host_memory_mib,
            host_storage_mib,
            host_gpu_available,
            host_network_available,
            allow_side_effect,
            offline,
        } => dispatch_plan_v3(
            pipeline,
            input,
            provider_registration,
            PipelinePlanningContext {
                host: HostResources {
                    cpu_threads: host_cpu_threads,
                    memory_mib: host_memory_mib,
                    storage_mib: host_storage_mib,
                    gpu_available: host_gpu_available,
                    network_available: host_network_available,
                },
                allowed_side_effects: allow_side_effect.into_iter().map(Into::into).collect(),
                offline,
            },
            presentation,
        ),
        Commands::Run {
            input,
            pipeline,
            output_directory,
        } => {
            let mut request = RunRequest::new(input, pipeline);
            if let Some(output_directory) = output_directory {
                request = request.with_output_directory(output_directory);
            }
            let cancellation = cli_cancellation_token()?;
            let outcome = if presentation == Presentation::Human {
                aniflow::run_with_progress_and_cancellation(request, &cancellation, print_progress)?
            } else {
                aniflow::run_with_progress_and_cancellation(request, &cancellation, |_| {})?
            };
            print_result(CommandName::Run, presentation, &outcome, || {
                print_outcome(&outcome);
            })
        }
        Commands::Resume { run_directory } => {
            let cancellation = cli_cancellation_token()?;
            let outcome = if presentation == Presentation::Human {
                aniflow::resume_with_progress_and_cancellation(
                    run_directory,
                    &cancellation,
                    print_progress,
                )?
            } else {
                aniflow::resume_with_progress_and_cancellation(
                    run_directory,
                    &cancellation,
                    |_| {},
                )?
            };
            print_result(CommandName::Resume, presentation, &outcome, || {
                print_outcome(&outcome);
            })
        }
        Commands::Status { run_directory } => {
            let status = aniflow::status(run_directory)?;
            print_result(CommandName::Status, presentation, &status, || {
                print_status(&status);
            })
        }
        Commands::Segment { command } => dispatch_segment(command, presentation),
    }
}

fn dispatch_plan_v3(
    pipeline: PathBuf,
    input_bindings: Vec<PipelineInputBinding>,
    provider_registrations: Vec<PathBuf>,
    context: PipelinePlanningContext,
    presentation: Presentation,
) -> CommandResult<()> {
    let plan = aniflow::plan_v3(pipeline, &input_bindings, &provider_registrations, &context)?;
    print_result(CommandName::PlanV3, presentation, &plan, || {
        print_pipeline_v3_plan(&plan);
    })
}

fn dispatch_segment(command: SegmentCommands, presentation: Presentation) -> CommandResult<()> {
    match command {
        SegmentCommands::Plan {
            input,
            output_directory,
            segment_duration_ms,
            mode,
            process_timeout_seconds,
        } => {
            let request =
                SegmentRequest::new(input, output_directory, segment_duration_ms, mode.into())
                    .with_process_timeout_seconds(process_timeout_seconds);
            let plan = aniflow::plan_segments(&request)?;
            print_result(CommandName::SegmentPlan, presentation, &plan, || {
                print_segment_plan(&plan);
            })
        }
        SegmentCommands::Run {
            input,
            output_directory,
            segment_duration_ms,
            mode,
            process_timeout_seconds,
        } => {
            let request =
                SegmentRequest::new(input, output_directory, segment_duration_ms, mode.into())
                    .with_process_timeout_seconds(process_timeout_seconds);
            let cancellation = cli_cancellation_token()?;
            let outcome = if presentation == Presentation::Human {
                aniflow::segment_with_progress(request, &cancellation, print_segment_progress)?
            } else {
                aniflow::segment_with_progress(request, &cancellation, |_| {})?
            };
            print_result(CommandName::SegmentRun, presentation, &outcome, || {
                print_segment_outcome(&outcome);
            })
        }
        SegmentCommands::Resume { run_directory } => {
            let cancellation = cli_cancellation_token()?;
            let outcome = if presentation == Presentation::Human {
                aniflow::resume_segments_with_progress(
                    &run_directory,
                    &cancellation,
                    print_segment_progress,
                )?
            } else {
                aniflow::resume_segments_with_progress(&run_directory, &cancellation, |_| {})?
            };
            print_result(CommandName::SegmentResume, presentation, &outcome, || {
                print_segment_outcome(&outcome);
            })
        }
        SegmentCommands::Reconstruct {
            run_directory,
            output_file,
        } => {
            let cancellation = cli_cancellation_token()?;
            let report = if presentation == Presentation::Human {
                aniflow::reconstruct_segments_with_progress(
                    &run_directory,
                    &output_file,
                    &cancellation,
                    print_segment_progress,
                )?
            } else {
                aniflow::reconstruct_segments_with_progress(
                    &run_directory,
                    &output_file,
                    &cancellation,
                    |_| {},
                )?
            };
            print_result(
                CommandName::SegmentReconstruct,
                presentation,
                &report,
                || print_reconstruction_report(&report),
            )
        }
    }
}

fn cli_cancellation_token() -> Result<aniflow::CancellationToken> {
    let cancellation = aniflow::CancellationToken::default();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel()).map_err(|error| {
        Error::new(
            ErrorCategory::Internal,
            format!("failed to install cancellation handler: {error}"),
        )
    })?;
    Ok(cancellation)
}

fn print_segment_plan(plan: &SegmentPlan) {
    println!("aniflow short-segment plan");
    println!("  source: {}", plan.source.display());
    println!("  segments: {}", plan.expected_segments);
    println!("  duration: {} ms", plan.segment_duration_ms);
    println!("  mode: {:?}", plan.mode);
    println!("  accuracy: {:?}", plan.boundary_accuracy);
    println!("  stream policy: {}", plan.stream_policy);
    println!("  estimated bytes: {}", plan.estimated_required_bytes);
}

fn print_segment_outcome(outcome: &SegmentOutcome) {
    println!("segment manifest: {}", outcome.manifest.display());
    println!("segments: {}", outcome.segment_count);
    println!("generated: {}", outcome.generated_segments);
    println!("reused: {}", outcome.resumed_segments);
}

fn print_reconstruction_report(report: &ReconstructionReport) {
    println!("reconstructed: {}", report.output.display());
    println!("validated: {}", report.validated);
    println!("duration delta: {} ms", report.duration_delta_ms);
    println!("tolerance: {} ms", report.tolerance_ms);
}

fn print_segment_progress(progress: &SegmentProgress) {
    match progress {
        SegmentProgress::Planned { segments } => println!("planned {segments} segments"),
        SegmentProgress::SegmentStarted { index, total } => {
            println!("segment {index}/{total}: running")
        }
        SegmentProgress::SegmentReused { index, total } => {
            println!("segment {index}/{total}: reused")
        }
        SegmentProgress::SegmentComplete { index, total } => {
            println!("segment {index}/{total}: complete")
        }
        SegmentProgress::ReconstructionStarted => println!("reconstruction: running"),
        SegmentProgress::ReconstructionComplete => println!("reconstruction: complete"),
        _ => {}
    }
}

fn print_result<T, F>(
    command: CommandName,
    presentation: Presentation,
    result: &T,
    print_human: F,
) -> CommandResult<()>
where
    T: Clone + Serialize,
    F: FnOnce(),
{
    match presentation {
        Presentation::Human => {
            print_human();
            Ok(())
        }
        Presentation::Machine => {
            print_json(&MachineEnvelope::success(command, result.clone())).map_err(Into::into)
        }
        Presentation::LegacyInspectJson => print_json(result).map_err(Into::into),
    }
}

fn print_error(
    command: CommandName,
    presentation: Presentation,
    failure: &CommandFailure,
) -> Result<()> {
    match presentation {
        Presentation::Human | Presentation::LegacyInspectJson => {
            eprintln!(
                "error: {}",
                escape_terminal_controls(&failure.error.to_string())
            );
            if let Some(planning) = &failure.planning {
                for diagnostic in &planning.diagnostics {
                    let mut location = String::new();
                    if let Some(stage) = &diagnostic.stage {
                        location.push_str(&format!(" stage={}", escape_terminal_controls(stage)));
                    }
                    if let Some(field) = &diagnostic.field {
                        location.push_str(&format!(" field={}", escape_terminal_controls(field)));
                    }
                    eprintln!("  diagnostic {:?}{location}", diagnostic.code);
                    for attempt in &diagnostic.attempts {
                        eprintln!(
                            "    {:?} {}: unavailable",
                            attempt.selection.source,
                            escape_terminal_controls(&attempt.candidate.registration_id)
                        );
                        for reason in &attempt.reasons {
                            eprintln!(
                                "      {:?}: {}",
                                reason.code,
                                escape_terminal_controls(&reason.detail)
                            );
                        }
                    }
                }
            }
            Ok(())
        }
        Presentation::Machine => {
            let rendered = if let Some(planning) = &failure.planning {
                render_json(&MachineEnvelope::failure(
                    command,
                    &failure.error,
                    Some(planning.clone()),
                ))?
            } else {
                render_json(&MachineEnvelope::<Value>::failure(
                    command,
                    &failure.error,
                    None,
                ))?
            };
            eprintln!("{rendered}");
            Ok(())
        }
    }
}

fn escape_terminal_controls(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}

fn print_json<T>(value: &T) -> Result<()>
where
    T: Serialize,
{
    println!("{}", render_json(value)?);
    Ok(())
}

fn render_json<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    serde_json::to_string_pretty(value).map_err(|error| {
        Error::new(
            ErrorCategory::Internal,
            format!("failed to serialize machine output: {error}"),
        )
    })
}

fn print_doctor(report: &DoctorReport) {
    println!("aniflow doctor");
    println!();
    for dependency in &report.dependencies {
        if let Some(summary) = &dependency.summary {
            println!("  {:<24} ok  {summary}", dependency.command);
        } else {
            println!("  {:<24} missing", dependency.command);
        }
    }
    if report.is_ready() {
        println!();
        println!("ready");
    }
}

fn print_inspection(inspection: &MediaInspection) {
    println!("aniflow inspect");
    println!();
    println!("  source       {}", inspection.source);
    println!("  duration     {:.3} seconds", inspection.duration_seconds);
    println!("  dimensions   {}x{}", inspection.width, inspection.height);
    println!(
        "  frame rate   {} ({:.6} fps)",
        inspection.average_frame_rate, inspection.frames_per_second
    );
    println!("  frames       ~{}", inspection.estimated_frame_count);
    println!("  video        {}", inspection.video_codec);
    println!(
        "  audio        {}",
        inspection.audio_codec.as_deref().unwrap_or("none")
    );
    println!(
        "  subtitles    {}",
        if inspection.has_subtitles {
            "yes"
        } else {
            "no"
        }
    );
}

fn print_plan(plan: &PipelinePlan) {
    println!("aniflow plan");
    println!();
    println!("source");
    println!("  path         {}", plan.inspection.source);
    println!(
        "  media        {}x{} @ {:.6} fps",
        plan.inspection.width, plan.inspection.height, plan.inspection.frames_per_second
    );
    println!("  frames       ~{}", plan.inspection.estimated_frame_count);
    println!();
    println!("pipeline");
    println!("  name         {}", plan.pipeline_name);
    println!("  version      {}", plan.pipeline_version);
    println!("  output       {}", plan.output_file.display());
    println!();
    println!("stages");
    for (index, stage) in plan.stages.iter().enumerate() {
        println!("  {:>2}. {stage}", index + 1);
    }
    if !plan.frame_processors.is_empty() {
        println!();
        println!("frame processors");
        for processor in &plan.frame_processors {
            println!(
                "  {:<20} {:<16} {}",
                processor.id, processor.command, processor.execution
            );
        }
    }
    println!();
    println!("required commands");
    for command in &plan.required_commands {
        println!("  {command}");
    }
}

fn print_pipeline_v3_plan(plan: &PipelineV3Plan) {
    println!("aniflow plan-v3");
    println!();
    println!("pipeline");
    println!("  name         {}", plan.payload.pipeline_name);
    println!("  schema       {}", plan.payload.pipeline_schema);
    println!("  digest       {}", plan.plan_sha256);
    println!();
    println!("inputs");
    for input in &plan.payload.inputs {
        println!(
            "  {:<20} {:?}, {} files, {} bytes, sha256:{}",
            input.id, input.kind, input.file_count, input.byte_count, input.content_sha256
        );
    }
    println!();
    println!("stages");
    for (index, stage) in plan.payload.stages.iter().enumerate() {
        println!(
            "  {:>2}. {:<20} {}@{} via {}",
            index + 1,
            stage.id,
            stage.capability.id,
            stage.capability.version,
            stage.provider_lock.payload.registration_id
        );
        for output in &stage.outputs {
            for artifact in &output.artifacts {
                println!(
                    "      {:<16} {} -> {}",
                    output.port, artifact.id, artifact.relative_path
                );
            }
        }
    }
    println!();
    println!("final outputs");
    for output in &plan.payload.outputs {
        println!("  {:<20} {}", output.id, output.artifact);
    }
}

fn print_progress(progress: &RunProgress) {
    match progress {
        RunProgress::Started {
            operation,
            run_directory,
            pipeline_name,
        } => {
            let command = match operation {
                RunOperation::Run => "run",
                RunOperation::Resume => "resume",
                _ => "run",
            };
            println!("aniflow {command}");
            println!();
            println!("  workspace    {}", run_directory.display());
            println!("  pipeline     {pipeline_name}");
            println!();
        }
        RunProgress::Stage { name, state } => {
            println!("  {name:<24} {}", progress_state(*state));
        }
        _ => {}
    }
}

fn print_outcome(outcome: &RunOutcome) {
    println!();
    println!("complete");
    println!("  output       {}", outcome.output.display());
    println!("  delivery     {}", outcome.delivery_manifest.display());
    println!("  manifest     {}", outcome.run_manifest.display());
}

fn print_status(status: &RunStatus) {
    println!("aniflow status");
    println!();
    println!("  run        {}", status.run_id);
    println!("  pipeline   {}", status.pipeline_name);
    println!("  source     {}", status.source_file.display());
    println!();
    println!("stages");
    for stage in &status.stages {
        println!("  {:<24} {}", stage.name, progress_state(stage.state));
        if let Some(message) = &stage.message {
            println!("    {message}");
        }
    }
    println!();
    println!("artifacts");
    for artifact in &status.artifacts {
        println!("  {:<24} {}", artifact.name, artifact.path.display());
    }
}

const fn progress_state(state: ProgressState) -> &'static str {
    match state {
        ProgressState::Waiting => "waiting",
        ProgressState::Cached => "cached",
        ProgressState::Running => "running",
        ProgressState::Complete => "complete",
        ProgressState::Failed => "failed",
        _ => "unknown",
    }
}
