use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use aniflow::audio_alignment::{
    self, AlignmentConfiguration, AlignmentRequest, AudioAlignmentFailure, AudioAlignmentPreflight,
};
use aniflow::audio_inspection::{
    self, AudioInspectionConfiguration, AudioInspectionFailure, AudioInspectionPreflight,
    AudioInspectionRequest,
};
use aniflow::audio_musical::{self, MusicalAnalysisConfiguration, MusicalAnalysisRequest};
use aniflow::audio_signal::{self, SignalAnalysisConfiguration, SignalAnalysisRequest};
use aniflow::audio_transcription::{
    self, AudioTranscriptionFailure, AudioTranscriptionPreflight, TranscriptionConfiguration,
    TranscriptionRequest,
};
use aniflow::timed_text::{
    self, ConversionLoss, ConversionLossKind, ConversionOptions, TimedTextFormat,
};
use aniflow::{
    CommandName, DoctorReport, Error, ErrorCategory, HostResources, MachineEnvelope,
    MediaInspection, PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1, PipelineInputBinding, PipelinePlan,
    PipelinePlanningContext, PipelinePlanningFailure, PipelineV3Plan, PipelineV3ProgressState,
    PipelineV3ResumeRequest, PipelineV3RunManifest, PipelineV3RunOutcome, PipelineV3RunProgress,
    PipelineV3RunRecovery, PipelineV3RunRequest, PipelineV3StageState, PipelineV3Workspace,
    ProgressState, ProviderExecutionLimits, ProviderRegistrationDocument, ProviderRegistry,
    ReconstructionReport, Result, RunOperation, RunOutcome, RunProgress, RunRequest, RunStatus,
    SegmentMode, SegmentOutcome, SegmentPlan, SegmentProgress, SegmentRequest, SideEffect,
};
use clap::{Args, Parser, Subcommand, ValueEnum};
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

#[derive(Debug, Clone, Copy, Args)]
struct ProviderLimitArguments {
    /// Maximum provider runtime before termination.
    #[arg(long, default_value_t = 21_600)]
    provider_timeout_seconds: u64,
    /// Grace period between provider termination and forced termination.
    #[arg(long, default_value_t = 2_000)]
    provider_termination_grace_milliseconds: u64,
    /// Maximum captured provider stdout bytes.
    #[arg(long, default_value_t = 67_108_864)]
    maximum_stdout_bytes: u64,
    /// Maximum captured provider stderr bytes.
    #[arg(long, default_value_t = 67_108_864)]
    maximum_stderr_bytes: u64,
    /// Maximum files accepted across provider artifacts.
    #[arg(long, default_value_t = 1_000_000)]
    maximum_artifact_files: u64,
    /// Maximum bytes accepted across provider artifacts.
    #[arg(long, default_value_t = 1_099_511_627_776)]
    maximum_artifact_bytes: u64,
}

#[derive(Debug, Clone, Copy, Args)]
struct AudioLimitArguments {
    #[arg(long, default_value_t = 600)]
    provider_timeout_seconds: u64,
    #[arg(long, default_value_t = 2_000)]
    provider_termination_grace_milliseconds: u64,
    #[arg(long, default_value_t = 1_048_576)]
    maximum_stdout_bytes: u64,
    #[arg(long, default_value_t = 1_048_576)]
    maximum_stderr_bytes: u64,
    #[arg(long)]
    maximum_artifact_files: Option<u64>,
    /// Maximum bytes including temporary staging; defaults to the selected audio profile.
    #[arg(long)]
    maximum_artifact_bytes: Option<u64>,
}

impl From<AudioLimitArguments> for ProviderExecutionLimits {
    fn from(arguments: AudioLimitArguments) -> Self {
        ProviderLimitArguments {
            provider_timeout_seconds: arguments.provider_timeout_seconds,
            provider_termination_grace_milliseconds: arguments
                .provider_termination_grace_milliseconds,
            maximum_stdout_bytes: arguments.maximum_stdout_bytes,
            maximum_stderr_bytes: arguments.maximum_stderr_bytes,
            maximum_artifact_files: arguments.maximum_artifact_files.unwrap_or(8),
            maximum_artifact_bytes: arguments.maximum_artifact_bytes.unwrap_or(288_358_400),
        }
        .into()
    }
}

impl AudioLimitArguments {
    fn alignment(self) -> ProviderExecutionLimits {
        let mut limits: ProviderExecutionLimits = self.into();
        limits.maximum_artifact_bytes = self
            .maximum_artifact_bytes
            .unwrap_or_else(|| audio_alignment::default_execution_limits().maximum_artifact_bytes);
        limits.maximum_artifact_files = self
            .maximum_artifact_files
            .unwrap_or_else(|| audio_alignment::default_execution_limits().maximum_artifact_files);
        limits
    }

    fn transcription(self) -> ProviderExecutionLimits {
        let mut limits: ProviderExecutionLimits = self.into();
        limits.maximum_artifact_bytes = self.maximum_artifact_bytes.unwrap_or_else(|| {
            audio_transcription::default_execution_limits().maximum_artifact_bytes
        });
        limits
    }
}

impl From<ProviderLimitArguments> for ProviderExecutionLimits {
    fn from(arguments: ProviderLimitArguments) -> Self {
        Self {
            timeout: Duration::from_secs(arguments.provider_timeout_seconds),
            termination_grace_period: Duration::from_millis(
                arguments.provider_termination_grace_milliseconds,
            ),
            maximum_stdout_bytes: arguments.maximum_stdout_bytes,
            maximum_stderr_bytes: arguments.maximum_stderr_bytes,
            maximum_artifact_files: arguments.maximum_artifact_files,
            maximum_artifact_bytes: arguments.maximum_artifact_bytes,
        }
    }
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
    planning: Option<Box<PipelinePlanningFailure>>,
    pipeline_v3_recovery: Option<Box<PipelineV3RunRecovery>>,
    audio_preflight: Option<Box<AudioInspectionPreflight>>,
    transcription_preflight: Option<Box<AudioTranscriptionPreflight>>,
    alignment_preflight: Option<Box<AudioAlignmentPreflight>>,
    timed_text_losses: Option<Vec<ConversionLoss>>,
}

impl From<Error> for CommandFailure {
    fn from(error: Error) -> Self {
        Self {
            error,
            planning: None,
            pipeline_v3_recovery: None,
            audio_preflight: None,
            transcription_preflight: None,
            alignment_preflight: None,
            timed_text_losses: None,
        }
    }
}

impl From<PipelinePlanningFailure> for CommandFailure {
    fn from(planning: PipelinePlanningFailure) -> Self {
        Self {
            error: Error::new(planning.category(), planning.message.clone()),
            planning: Some(Box::new(planning)),
            pipeline_v3_recovery: None,
            audio_preflight: None,
            transcription_preflight: None,
            alignment_preflight: None,
            timed_text_losses: None,
        }
    }
}

impl CommandFailure {
    fn pipeline_v3(error: Error, run_directory: Option<PathBuf>) -> Self {
        Self {
            error,
            planning: None,
            pipeline_v3_recovery: run_directory.map(pipeline_v3_recovery).map(Box::new),
            audio_preflight: None,
            transcription_preflight: None,
            alignment_preflight: None,
            timed_text_losses: None,
        }
    }
}

impl From<AudioInspectionFailure> for CommandFailure {
    fn from(failure: AudioInspectionFailure) -> Self {
        Self {
            error: failure.error,
            planning: failure.planning,
            pipeline_v3_recovery: None,
            audio_preflight: failure.preflight,
            transcription_preflight: None,
            alignment_preflight: None,
            timed_text_losses: None,
        }
    }
}

impl From<AudioTranscriptionFailure> for CommandFailure {
    fn from(failure: AudioTranscriptionFailure) -> Self {
        Self {
            error: failure.error,
            planning: failure.planning,
            pipeline_v3_recovery: None,
            audio_preflight: failure.inspection_preflight,
            transcription_preflight: failure.preflight,
            alignment_preflight: None,
            timed_text_losses: None,
        }
    }
}

impl From<AudioAlignmentFailure> for CommandFailure {
    fn from(failure: AudioAlignmentFailure) -> Self {
        Self {
            error: failure.error,
            planning: failure.planning,
            pipeline_v3_recovery: None,
            audio_preflight: failure.inspection_preflight,
            transcription_preflight: None,
            alignment_preflight: failure.preflight,
            timed_text_losses: None,
        }
    }
}

impl From<timed_text::ConversionFailure> for CommandFailure {
    fn from(failure: timed_text::ConversionFailure) -> Self {
        Self {
            error: failure.error,
            planning: None,
            pipeline_v3_recovery: None,
            audio_preflight: None,
            transcription_preflight: None,
            alignment_preflight: None,
            timed_text_losses: Some(failure.losses),
        }
    }
}

#[derive(Debug, Subcommand)]
enum TimedTextCommands {
    /// List the bounded read/write subsets supported by this version.
    Formats,
    /// Convert synthetic or explicitly selected text into a new output package.
    Convert {
        /// Source text or normalized JSON document; never modified.
        #[arg(long)]
        input: PathBuf,
        /// Source format: plain, lrc, srt, webvtt, ttml, or json.
        #[arg(long)]
        from: String,
        /// Target format: plain, lrc, srt, webvtt, ttml, or json.
        #[arg(long)]
        to: String,
        /// Exact new directory to create; existing paths are refused.
        #[arg(long)]
        output_directory: PathBuf,
        /// Bounded import-context JSON with explicit provenance and optional audio binding.
        #[arg(long)]
        context: Option<PathBuf>,
        /// Explicit allowed loss kinds, comma-separated or repeated (snake_case).
        #[arg(long, value_delimiter = ',')]
        allow_loss: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Convert lyrics and timed text with explicit loss reports.
    TimedText {
        #[command(subcommand)]
        command: TimedTextCommands,
    },
    /// Inspect bounded PCM16 WAV sources with explicitly pinned local tools.
    Audio {
        #[command(subcommand)]
        command: AudioCommands,
    },
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
    /// Plan and execute a new content-aware Pipeline v3 run.
    RunV3 {
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
        /// Authorize one provider side effect during execution.
        #[arg(long, value_enum)]
        allow_side_effect: Vec<SideEffectArgument>,
        /// Reject network-dependent providers.
        #[arg(long)]
        offline: bool,
        /// Parent directory for the new Pipeline v3 run.
        #[arg(long)]
        output_directory: Option<PathBuf>,
        #[command(flatten)]
        provider_limits: ProviderLimitArguments,
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
    /// Resume a Pipeline v3 run from compatible content-addressed checkpoints.
    ResumeV3 {
        /// Existing Pipeline v3 run directory.
        run_directory: PathBuf,
        /// Rebind one declared input artifact to a local file or directory.
        #[arg(
            long,
            required = true,
            value_name = "ARTIFACT_ID=PATH",
            value_parser = parse_pipeline_input_binding
        )]
        input: Vec<PipelineInputBinding>,
        /// Explicit portable provider registration document.
        #[arg(long, required = true)]
        provider_registration: Vec<PathBuf>,
        #[command(flatten)]
        provider_limits: ProviderLimitArguments,
    },
    /// Display stage and artifact status for a run.
    Status {
        /// Existing aniflow run directory.
        run_directory: PathBuf,
    },
    /// Display immutable Pipeline v3 run state without modifying the workspace.
    StatusV3 {
        /// Existing Pipeline v3 run directory.
        run_directory: PathBuf,
    },
    /// Plan, execute, resume, or reconstruct a short-segment workflow.
    Segment {
        #[command(subcommand)]
        command: SegmentCommands,
    },
}

#[derive(Debug, Args)]
struct AudioSourceArguments {
    /// Immutable PCM16 WAV source (original mix when selecting a stem).
    #[arg(long)]
    input: PathBuf,
    /// JSON configuration with exact FFmpeg/ffprobe paths, versions, and digests.
    #[arg(long)]
    configuration: PathBuf,
    /// Completed Pipeline v3 separation workspace containing the selected stem.
    #[arg(long, requires_all = ["stem_stage", "stem_id"])]
    stem_run: Option<PathBuf>,
    /// Exact accepted separation stage identity.
    #[arg(long, requires = "stem_run")]
    stem_stage: Option<String>,
    /// Exact declared stem artifact identity; no inferred instrument names.
    #[arg(long, requires = "stem_run")]
    stem_id: Option<String>,
    /// Explicit zero-based channels. This profile requires all channels in order.
    #[arg(long, value_delimiter = ',', num_args = 1, requires = "stem_run")]
    stem_channels: Option<Vec<u16>>,
    /// Start of the selected half-open stem frame range; currently must be zero.
    #[arg(long, requires_all = ["stem_run", "stem_end_frame"])]
    stem_start_frame: Option<u64>,
    /// Exclusive end of the range; currently must equal the full stem frame count.
    #[arg(long, requires_all = ["stem_run", "stem_start_frame"])]
    stem_end_frame: Option<u64>,
    /// Maximum duration difference in milliseconds (0–20; defaults to 20).
    #[arg(long, value_parser = clap::value_parser!(u16).range(0..=20), requires = "stem_run")]
    stem_duration_tolerance_milliseconds: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AudioAnalysisKind {
    Technical,
    Signal,
    Musical,
    Transcription,
    LyricsAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AudioEstimateKind {
    Signal,
    Musical,
}

#[derive(Debug, Args)]
struct AudioAnalysisSelectionArguments {
    /// Select technical, signal, musical, transcription, or reviewed-lyrics alignment.
    #[arg(long, value_enum, default_value_t = AudioAnalysisKind::Technical)]
    analysis: AudioAnalysisKind,
    /// Versioned signal settings, required when --analysis signal is selected.
    #[arg(
        long,
        required_if_eq("analysis", "signal"),
        conflicts_with_all = ["musical_configuration", "transcription_configuration", "alignment_configuration", "lyrics"]
    )]
    signal_configuration: Option<PathBuf>,
    /// Exact local analyzer settings, required for --analysis musical.
    #[arg(
        long,
        required_if_eq("analysis", "musical"),
        conflicts_with_all = ["signal_configuration", "transcription_configuration", "alignment_configuration", "lyrics"]
    )]
    musical_configuration: Option<PathBuf>,
    /// Pinned local speech tool/model settings, required for transcription.
    #[arg(long, required_if_eq("analysis", "transcription"), conflicts_with_all = ["signal_configuration", "musical_configuration", "alignment_configuration", "lyrics"])]
    transcription_configuration: Option<PathBuf>,
    /// Pinned offline alignment configuration, required for lyrics-alignment.
    #[arg(long, required_if_eq("analysis", "lyrics-alignment"), requires = "lyrics", conflicts_with_all = ["signal_configuration", "musical_configuration", "transcription_configuration"])]
    alignment_configuration: Option<PathBuf>,
    /// Explicitly reviewed timed-text JSON; never modified.
    #[arg(
        long,
        required_if_eq("analysis", "lyrics-alignment"),
        requires = "alignment_configuration"
    )]
    lyrics: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum AudioCommands {
    /// Propose alignment timing for reviewed lyrics and export candidate cues.
    Lyrics {
        #[command(subcommand)]
        command: LyricsCommands,
    },
    /// Observe English transcript segments with an explicitly pinned offline provider.
    Transcribe {
        #[command(flatten)]
        source: AudioSourceArguments,
        #[arg(long)]
        transcription_configuration: PathBuf,
        #[arg(long)]
        output_directory: Option<PathBuf>,
        #[command(flatten)]
        provider_limits: AudioLimitArguments,
    },
    /// Export observed transcript cues using the loss-aware timed-text converter.
    TranscriptExport {
        /// Validated transcription report; source bytes remain unchanged.
        #[arg(long)]
        transcription: PathBuf,
        /// Target: plain, lrc, srt, webvtt, ttml, or json.
        #[arg(long)]
        to: String,
        /// New output package directory beneath an existing canonical parent.
        #[arg(long)]
        output_directory: PathBuf,
        /// Explicit permission for each reported loss kind.
        #[arg(long, value_delimiter = ',')]
        allow_loss: Vec<String>,
    },
    /// Preflight pinned tools and resolve a plan without creating a workspace.
    Plan {
        #[command(flatten)]
        source: AudioSourceArguments,
        #[command(flatten)]
        selection: AudioAnalysisSelectionArguments,
    },
    /// Inspect and decode a private source snapshot into versioned evidence.
    Inspect {
        #[command(flatten)]
        source: AudioSourceArguments,
        /// Parent directory for the isolated inspection run.
        #[arg(long)]
        output_directory: Option<PathBuf>,
        #[command(flatten)]
        provider_limits: AudioLimitArguments,
    },
    /// Analyze immutable PCM16 audio with the selected pinned local provider.
    Analyze {
        #[command(flatten)]
        source: AudioSourceArguments,
        /// Select signal measurements or musical estimates.
        #[arg(long, value_enum, default_value_t = AudioEstimateKind::Signal)]
        analysis: AudioEstimateKind,
        /// Versioned thresholds and window settings for signal measurements.
        #[arg(
            long,
            required_if_eq("analysis", "signal"),
            conflicts_with = "musical_configuration"
        )]
        signal_configuration: Option<PathBuf>,
        /// Exact local analyzer settings, required for musical estimates.
        #[arg(
            long,
            required_if_eq("analysis", "musical"),
            conflicts_with = "signal_configuration"
        )]
        musical_configuration: Option<PathBuf>,
        /// Parent directory for the isolated two-stage analysis run.
        #[arg(long)]
        output_directory: Option<PathBuf>,
        #[command(flatten)]
        provider_limits: AudioLimitArguments,
    },
    /// Recheck pinned tools and reuse only compatible inspection checkpoints.
    Resume {
        /// Existing inspection run directory. Use status-v3 for read-only status.
        run_directory: PathBuf,
        #[command(flatten)]
        source: AudioSourceArguments,
        #[command(flatten)]
        selection: AudioAnalysisSelectionArguments,
        #[command(flatten)]
        provider_limits: AudioLimitArguments,
    },
}

#[derive(Debug, Subcommand)]
enum LyricsCommands {
    /// Align explicitly reviewed text with a pinned offline provider.
    Align {
        #[command(flatten)]
        source: AudioSourceArguments,
        #[arg(long)]
        lyrics: PathBuf,
        #[arg(long)]
        alignment_configuration: PathBuf,
        #[arg(long)]
        output_directory: Option<PathBuf>,
        #[command(flatten)]
        provider_limits: AudioLimitArguments,
    },
    /// Export proposed timing while preserving reviewed text and reporting losses.
    Export {
        #[arg(long)]
        alignment: PathBuf,
        #[arg(long)]
        to: String,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long, value_delimiter = ',')]
        allow_loss: Vec<String>,
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
            Self::TimedText { command } => match command {
                TimedTextCommands::Formats => CommandName::TimedTextFormats,
                TimedTextCommands::Convert { .. } => CommandName::TimedTextConvert,
            },
            Self::Audio { command } => match command {
                AudioCommands::Lyrics { command } => match command {
                    LyricsCommands::Align { .. } => CommandName::AudioLyricsAlign,
                    LyricsCommands::Export { .. } => CommandName::AudioLyricsExport,
                },
                AudioCommands::Plan { .. } => CommandName::AudioPlan,
                AudioCommands::Inspect { .. } => CommandName::AudioInspect,
                AudioCommands::Analyze { .. } => CommandName::AudioAnalyze,
                AudioCommands::Resume { .. } => CommandName::AudioResume,
                AudioCommands::Transcribe { .. } => CommandName::AudioTranscribe,
                AudioCommands::TranscriptExport { .. } => CommandName::AudioTranscriptExport,
            },
            Self::Doctor { .. } => CommandName::Doctor,
            Self::Inspect { .. } => CommandName::Inspect,
            Self::Plan { .. } => CommandName::Plan,
            Self::PlanV3 { .. } => CommandName::PlanV3,
            Self::RunV3 { .. } => CommandName::RunV3,
            Self::Run { .. } => CommandName::Run,
            Self::Resume { .. } => CommandName::Resume,
            Self::ResumeV3 { .. } => CommandName::ResumeV3,
            Self::Status { .. } => CommandName::Status,
            Self::StatusV3 { .. } => CommandName::StatusV3,
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
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments
        .get(1)
        .is_some_and(|argument| argument == aniflow::PROVIDER_INVOCATION_ARGUMENT)
    {
        let result = if arguments.len() == 3 {
            audio_inspection::execute_provider_invocation(PathBuf::from(&arguments[2]))
        } else {
            Err(Error::new(
                ErrorCategory::Configuration,
                "expected exactly --aniflow-invocation <absolute-request-path>",
            ))
        };
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {}", escape_terminal_controls(&error.to_string()));
                ExitCode::from(error.category().exit_code())
            }
        };
    }
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
        Commands::TimedText { command } => dispatch_timed_text(command, presentation),
        Commands::Audio { command } => dispatch_audio(command, presentation),
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
            pipeline_v3_context(
                host_cpu_threads,
                host_memory_mib,
                host_storage_mib,
                host_gpu_available,
                host_network_available,
                allow_side_effect,
                offline,
            ),
            presentation,
        ),
        Commands::RunV3 {
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
            output_directory,
            provider_limits,
        } => {
            let context = pipeline_v3_context(
                host_cpu_threads,
                host_memory_mib,
                host_storage_mib,
                host_gpu_available,
                host_network_available,
                allow_side_effect,
                offline,
            );
            let plan = aniflow::plan_v3(pipeline, &input, &provider_registration, &context)?;
            let registry = load_provider_registry(&provider_registration)?;
            let mut request = PipelineV3RunRequest::new(plan, input, registry)
                .with_execution_limits(provider_limits.into());
            if let Some(output_directory) = output_directory {
                request = request.with_output_directory(output_directory);
            }
            let cancellation = cli_cancellation_token()?;
            let mut recovery_run_directory = None;
            let outcome = aniflow::run_v3_with_progress_and_cancellation(
                request,
                &cancellation,
                |progress| {
                    observe_pipeline_v3_progress(
                        presentation,
                        &mut recovery_run_directory,
                        progress,
                    );
                },
            )
            .map_err(|error| CommandFailure::pipeline_v3(error, recovery_run_directory))?;
            print_result(CommandName::RunV3, presentation, &outcome, || {
                print_pipeline_v3_outcome(&outcome);
            })
        }
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
        Commands::ResumeV3 {
            run_directory,
            input,
            provider_registration,
            provider_limits,
        } => {
            let registry = load_provider_registry(&provider_registration)?;
            let request = PipelineV3ResumeRequest::new(run_directory, input, registry)
                .with_execution_limits(provider_limits.into());
            let cancellation = cli_cancellation_token()?;
            let mut recovery_run_directory = None;
            let outcome = aniflow::resume_v3_with_progress_and_cancellation(
                request,
                &cancellation,
                |progress| {
                    observe_pipeline_v3_progress(
                        presentation,
                        &mut recovery_run_directory,
                        progress,
                    );
                },
            )
            .map_err(|error| CommandFailure::pipeline_v3(error, recovery_run_directory))?;
            print_result(CommandName::ResumeV3, presentation, &outcome, || {
                print_pipeline_v3_outcome(&outcome);
            })
        }
        Commands::Status { run_directory } => {
            let status = aniflow::status(run_directory)?;
            print_result(CommandName::Status, presentation, &status, || {
                print_status(&status);
            })
        }
        Commands::StatusV3 { run_directory } => {
            let status = aniflow::status_v3(run_directory)?;
            print_result(CommandName::StatusV3, presentation, &status, || {
                print_pipeline_v3_status(&status);
            })
        }
        Commands::Segment { command } => dispatch_segment(command, presentation),
    }
}

fn parse_text_format(value: &str) -> Result<TimedTextFormat> {
    let canonical = match value {
        "text" => "plain",
        "vtt" => "webvtt",
        other => other,
    };
    canonical.parse().map_err(|_| {
        Error::new(
            ErrorCategory::Configuration,
            "unsupported timed-text format; use plain, lrc, srt, webvtt, ttml, or json",
        )
    })
}

fn dispatch_timed_text(
    command: TimedTextCommands,
    presentation: Presentation,
) -> CommandResult<()> {
    match command {
        TimedTextCommands::Formats => {
            let registry = timed_text::registry();
            print_result(
                CommandName::TimedTextFormats,
                presentation,
                &registry,
                || {
                    for profile in &registry.formats {
                        println!(
                            "{} (.{}): {}",
                            profile.format.as_str(),
                            profile.extension,
                            profile.subset
                        );
                    }
                },
            )
        }
        TimedTextCommands::Convert {
            input,
            from,
            to,
            output_directory,
            context,
            allow_loss,
        } => {
            let result = (|| -> std::result::Result<_, timed_text::ConversionFailure> {
                let from = parse_text_format(&from)?;
                let to = parse_text_format(&to)?;
                let context: Option<timed_text::ImportContext> = context
                    .as_deref()
                    .map(|path| {
                        let bytes = read_audio_configuration(path, "timed-text import context")?;
                        timed_text::ImportContext::from_json_slice(&bytes)
                    })
                    .transpose()?;
                let allow_losses = allow_loss.iter().map(|value| {
                    value.replace('-', "_").parse::<ConversionLossKind>()
                        .map_err(|_| Error::new(ErrorCategory::Configuration,
                            "unsupported loss kind; use timing, end_times, precision, cue_identifiers, speaker, metadata, or language"))
                }).collect::<Result<Vec<_>>>()?;
                timed_text::convert_file(
                    &input,
                    from,
                    to,
                    &output_directory,
                    context.as_ref(),
                    &ConversionOptions { allow_losses },
                )
            })()?;
            print_result(CommandName::TimedTextConvert, presentation, &result, || {
                println!(
                    "Converted payload: {}",
                    escape_terminal_controls(&result.payload_path.display().to_string())
                );
                println!(
                    "Completion report: {}",
                    escape_terminal_controls(&result.report_path.display().to_string())
                );
                println!("Reported losses: {}", result.report.losses.len());
            })
        }
    }
}

fn read_audio_configuration(path: &std::path::Path, label: &str) -> Result<Vec<u8>> {
    use std::io::Read as _;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| {
        Error::new(
            ErrorCategory::Configuration,
            format!("{label} is unavailable"),
        )
    })?;
    if !metadata.is_file() || metadata.len() > 65_536 {
        return Err(Error::new(
            ErrorCategory::Configuration,
            format!("{label} must be a regular file no larger than 64 KiB"),
        ));
    }
    let file = std::fs::File::open(path).map_err(|_| {
        Error::new(
            ErrorCategory::Configuration,
            format!("{label} is unavailable"),
        )
    })?;
    let mut bytes = Vec::new();
    file.take(65_537).read_to_end(&mut bytes).map_err(|_| {
        Error::new(
            ErrorCategory::Configuration,
            format!("{label} cannot be read"),
        )
    })?;
    if bytes.len() > 65_536 {
        return Err(Error::new(
            ErrorCategory::Configuration,
            format!("{label} exceeds 64 KiB"),
        ));
    }
    Ok(bytes)
}

fn audio_request(source: AudioSourceArguments) -> Result<AudioInspectionRequest> {
    let bytes = read_audio_configuration(&source.configuration, "audio inspection configuration")?;
    let configuration = AudioInspectionConfiguration::from_json_slice(&bytes)?;
    let executable = std::env::current_exe().map_err(|_| {
        Error::new(
            ErrorCategory::Internal,
            "cannot locate the native aniflow provider executable",
        )
    })?;
    let mut request = AudioInspectionRequest::new(source.input, configuration, executable);
    if let Some(run_directory) = source.stem_run {
        let selection = aniflow::audio_stem::StemSelection {
            run_directory,
            stage_id: source.stem_stage.ok_or_else(|| {
                Error::new(
                    ErrorCategory::Configuration,
                    "stem selection requires --stem-stage",
                )
            })?,
            stem_id: source.stem_id.ok_or_else(|| {
                Error::new(
                    ErrorCategory::Configuration,
                    "stem selection requires --stem-id",
                )
            })?,
            channels: source.stem_channels,
            range: match (source.stem_start_frame, source.stem_end_frame) {
                (Some(start), Some(end)) => {
                    Some(aniflow::audio_analysis::AudioFrameRange { start, end })
                }
                (None, None) => None,
                _ => {
                    return Err(Error::new(
                        ErrorCategory::Configuration,
                        "stem range requires both frame boundaries",
                    ));
                }
            },
            duration_tolerance_milliseconds: source
                .stem_duration_tolerance_milliseconds
                .unwrap_or(20),
        };
        request = request.with_stem_selection(selection);
    }
    Ok(request)
}

fn signal_settings(path: &std::path::Path) -> Result<SignalAnalysisConfiguration> {
    SignalAnalysisConfiguration::from_json_slice(&read_audio_configuration(
        path,
        "audio signal configuration",
    )?)
}

fn transcription_settings(path: &std::path::Path) -> Result<TranscriptionConfiguration> {
    TranscriptionConfiguration::from_json_slice(&read_audio_configuration(
        path,
        "audio transcription configuration",
    )?)
}

fn alignment_settings(path: &std::path::Path) -> Result<AlignmentConfiguration> {
    AlignmentConfiguration::from_json_slice(&read_audio_configuration(
        path,
        "audio alignment configuration",
    )?)
}

enum SelectedAudioAnalysis {
    Technical,
    Signal(SignalAnalysisConfiguration),
    Musical(MusicalAnalysisConfiguration),
    Transcription(TranscriptionConfiguration),
    LyricsAlignment(AlignmentConfiguration, PathBuf),
}

fn selected_audio_settings(
    selection: AudioAnalysisSelectionArguments,
) -> Result<SelectedAudioAnalysis> {
    match (
        selection.analysis,
        selection.signal_configuration,
        selection.musical_configuration,
        selection.transcription_configuration,
        selection.alignment_configuration,
        selection.lyrics,
    ) {
        (AudioAnalysisKind::Technical, None, None, None, None, None) => {
            Ok(SelectedAudioAnalysis::Technical)
        }
        (AudioAnalysisKind::Signal, Some(path), None, None, None, None) => {
            signal_settings(&path).map(SelectedAudioAnalysis::Signal)
        }
        (AudioAnalysisKind::Musical, None, Some(path), None, None, None) => {
            MusicalAnalysisConfiguration::from_json_slice(&read_audio_configuration(
                &path,
                "audio musical configuration",
            )?)
            .map(SelectedAudioAnalysis::Musical)
        }
        (AudioAnalysisKind::Transcription, None, None, Some(path), None, None) => {
            transcription_settings(&path).map(SelectedAudioAnalysis::Transcription)
        }
        (AudioAnalysisKind::LyricsAlignment, None, None, None, Some(path), Some(lyrics)) => Ok(
            SelectedAudioAnalysis::LyricsAlignment(alignment_settings(&path)?, lyrics),
        ),
        _ => Err(Error::new(
            ErrorCategory::Configuration,
            "analysis selection requires exactly its matching configuration; technical takes none",
        )),
    }
}

fn dispatch_audio(command: AudioCommands, presentation: Presentation) -> CommandResult<()> {
    let cancellation = cli_cancellation_token()?;
    match command {
        AudioCommands::Lyrics { command } => dispatch_lyrics(command, presentation, &cancellation),
        AudioCommands::Transcribe {
            source,
            transcription_configuration,
            output_directory,
            provider_limits,
        } => {
            let inspection =
                audio_request(source)?.with_execution_limits(provider_limits.transcription());
            let request = TranscriptionRequest::new(
                inspection,
                transcription_settings(&transcription_configuration)?,
            );
            let mut recovery_run_directory = None;
            let outcome =
                audio_transcription::run(request, output_directory, &cancellation, |progress| {
                    observe_pipeline_v3_progress(
                        presentation,
                        &mut recovery_run_directory,
                        progress,
                    );
                })
                .map_err(|failure| audio_failure_with_recovery(failure, recovery_run_directory))?;
            print_result(CommandName::AudioTranscribe, presentation, &outcome, || {
                print_pipeline_v3_outcome(&outcome)
            })
        }
        AudioCommands::TranscriptExport {
            transcription,
            to,
            output_directory,
            allow_loss,
        } => {
            let to = parse_text_format(&to)?;
            let allow_losses = allow_loss
                .iter()
                .map(|value| {
                    value
                        .replace('-', "_")
                        .parse::<ConversionLossKind>()
                        .map_err(|_| {
                            Error::new(
                                ErrorCategory::Configuration,
                                "unsupported timed-text loss kind",
                            )
                        })
                })
                .collect::<Result<Vec<_>>>()?;
            let outcome = audio_transcription::export_transcript_file(
                &transcription,
                to,
                &output_directory,
                &ConversionOptions { allow_losses },
            )?;
            print_result(
                CommandName::AudioTranscriptExport,
                presentation,
                &outcome,
                || {
                    println!(
                        "Transcript payload: {}",
                        escape_terminal_controls(
                            &outcome.conversion.payload_path.display().to_string()
                        )
                    );
                    println!(
                        "Completion report: {}",
                        escape_terminal_controls(
                            &outcome.conversion.report_path.display().to_string()
                        )
                    );
                    println!(
                        "Reported losses: {}",
                        outcome.conversion.report.losses.len()
                    );
                },
            )
        }
        AudioCommands::Plan { source, selection } => {
            let request = audio_request(source)?;
            let plan = match selected_audio_settings(selection)? {
                SelectedAudioAnalysis::Signal(settings) => audio_signal::plan(
                    &SignalAnalysisRequest::new(request, settings),
                    &cancellation,
                )?,
                SelectedAudioAnalysis::Musical(settings) => audio_musical::plan(
                    &MusicalAnalysisRequest::new(request, settings),
                    &cancellation,
                )?,
                SelectedAudioAnalysis::Transcription(settings) => audio_transcription::plan(
                    &TranscriptionRequest::new(request, settings),
                    &cancellation,
                )?,
                SelectedAudioAnalysis::LyricsAlignment(settings, lyrics) => audio_alignment::plan(
                    &AlignmentRequest::new(request, settings, lyrics),
                    &cancellation,
                )?,
                SelectedAudioAnalysis::Technical => {
                    audio_inspection::plan(&request, &cancellation)?
                }
            };
            print_result(CommandName::AudioPlan, presentation, &plan, || {
                print_pipeline_v3_plan(&plan)
            })
        }
        AudioCommands::Inspect {
            source,
            output_directory,
            provider_limits,
        } => {
            let request = audio_request(source)?.with_execution_limits(provider_limits.into());
            let mut recovery_run_directory = None;
            let outcome =
                audio_inspection::run(request, output_directory, &cancellation, |progress| {
                    observe_pipeline_v3_progress(
                        presentation,
                        &mut recovery_run_directory,
                        progress,
                    );
                })
                .map_err(|failure| audio_failure_with_recovery(failure, recovery_run_directory))?;
            print_result(CommandName::AudioInspect, presentation, &outcome, || {
                print_pipeline_v3_outcome(&outcome)
            })
        }
        AudioCommands::Resume {
            run_directory,
            source,
            selection,
            provider_limits,
        } => {
            let limits = if selection.analysis == AudioAnalysisKind::Transcription {
                provider_limits.transcription()
            } else if selection.analysis == AudioAnalysisKind::LyricsAlignment {
                provider_limits.alignment()
            } else {
                provider_limits.into()
            };
            let request = audio_request(source)?.with_execution_limits(limits);
            let mut recovery_run_directory = None;
            let progress = |progress: &PipelineV3RunProgress| {
                observe_pipeline_v3_progress(presentation, &mut recovery_run_directory, progress);
            };
            let outcome = match selected_audio_settings(selection)? {
                SelectedAudioAnalysis::Signal(settings) => audio_signal::resume(
                    SignalAnalysisRequest::new(request, settings),
                    run_directory,
                    &cancellation,
                    progress,
                )
                .map_err(CommandFailure::from),
                SelectedAudioAnalysis::Musical(settings) => audio_musical::resume(
                    MusicalAnalysisRequest::new(request, settings),
                    run_directory,
                    &cancellation,
                    progress,
                )
                .map_err(CommandFailure::from),
                SelectedAudioAnalysis::Transcription(settings) => audio_transcription::resume(
                    TranscriptionRequest::new(request, settings),
                    run_directory,
                    &cancellation,
                    progress,
                )
                .map_err(CommandFailure::from),
                SelectedAudioAnalysis::LyricsAlignment(settings, lyrics) => {
                    audio_alignment::resume(
                        AlignmentRequest::new(request, settings, lyrics),
                        run_directory,
                        &cancellation,
                        progress,
                    )
                    .map_err(CommandFailure::from)
                }
                SelectedAudioAnalysis::Technical => {
                    audio_inspection::resume(request, run_directory, &cancellation, progress)
                        .map_err(CommandFailure::from)
                }
            }
            .map_err(|failure| audio_failure_with_recovery(failure, recovery_run_directory))?;
            print_result(CommandName::AudioResume, presentation, &outcome, || {
                print_pipeline_v3_outcome(&outcome)
            })
        }
        AudioCommands::Analyze {
            source,
            analysis,
            signal_configuration,
            musical_configuration,
            output_directory,
            provider_limits,
        } => {
            let inspection = audio_request(source)?.with_execution_limits(provider_limits.into());
            let settings = selected_audio_settings(AudioAnalysisSelectionArguments {
                analysis: match analysis {
                    AudioEstimateKind::Signal => AudioAnalysisKind::Signal,
                    AudioEstimateKind::Musical => AudioAnalysisKind::Musical,
                },
                signal_configuration,
                musical_configuration,
                transcription_configuration: None,
                alignment_configuration: None,
                lyrics: None,
            })?;
            let mut recovery_run_directory = None;
            let progress = |progress: &PipelineV3RunProgress| {
                observe_pipeline_v3_progress(presentation, &mut recovery_run_directory, progress);
            };
            let outcome = match settings {
                SelectedAudioAnalysis::Signal(settings) => audio_signal::run(
                    SignalAnalysisRequest::new(inspection, settings), output_directory, &cancellation, progress,
                ),
                SelectedAudioAnalysis::Musical(settings) => audio_musical::run(
                    MusicalAnalysisRequest::new(inspection, settings), output_directory, &cancellation, progress,
                ),
                SelectedAudioAnalysis::Technical | SelectedAudioAnalysis::Transcription(_) | SelectedAudioAnalysis::LyricsAlignment(_, _) => return Err(Error::new(
                    ErrorCategory::Configuration, "audio analyze requires --analysis signal or musical; use audio inspect for technical inspection",
                ).into()),
            }
            .map_err(|failure| audio_failure_with_recovery(failure, recovery_run_directory))?;
            print_result(CommandName::AudioAnalyze, presentation, &outcome, || {
                print_pipeline_v3_outcome(&outcome)
            })
        }
    }
}

fn dispatch_lyrics(
    command: LyricsCommands,
    presentation: Presentation,
    cancellation: &aniflow::CancellationToken,
) -> CommandResult<()> {
    match command {
        LyricsCommands::Align {
            source,
            lyrics,
            alignment_configuration,
            output_directory,
            provider_limits,
        } => {
            let inspection =
                audio_request(source)?.with_execution_limits(provider_limits.alignment());
            let request = AlignmentRequest::new(
                inspection,
                alignment_settings(&alignment_configuration)?,
                lyrics,
            );
            let mut recovery_run_directory = None;
            let outcome =
                audio_alignment::run(request, output_directory, cancellation, |progress| {
                    observe_pipeline_v3_progress(
                        presentation,
                        &mut recovery_run_directory,
                        progress,
                    );
                })
                .map_err(|failure| audio_failure_with_recovery(failure, recovery_run_directory))?;
            print_result(
                CommandName::AudioLyricsAlign,
                presentation,
                &outcome,
                || print_pipeline_v3_outcome(&outcome),
            )
        }
        LyricsCommands::Export {
            alignment,
            to,
            output_directory,
            allow_loss,
        } => {
            let to = parse_text_format(&to)?;
            let allow_losses = allow_loss
                .iter()
                .map(|value| value.replace('-', "_").parse::<ConversionLossKind>())
                .collect::<Result<Vec<_>>>()?;
            let outcome = audio_alignment::export_alignment_file(
                &alignment,
                to,
                &output_directory,
                &ConversionOptions { allow_losses },
            )?;
            print_result(
                CommandName::AudioLyricsExport,
                presentation,
                &outcome,
                || {
                    println!(
                        "Candidate timing payload: {}",
                        escape_terminal_controls(
                            &outcome.conversion.payload_path.display().to_string()
                        )
                    );
                    println!(
                        "Completion report: {}",
                        escape_terminal_controls(
                            &outcome.conversion.report_path.display().to_string()
                        )
                    );
                    println!(
                        "Reported losses: {}",
                        outcome.conversion.report.losses.len()
                    );
                },
            )
        }
    }
}

fn audio_failure_with_recovery(
    failure: impl Into<CommandFailure>,
    run_directory: Option<PathBuf>,
) -> CommandFailure {
    let mut failure = failure.into();
    if failure.audio_preflight.is_none()
        && failure.transcription_preflight.is_none()
        && failure.alignment_preflight.is_none()
        && failure.planning.is_none()
    {
        failure.pipeline_v3_recovery = run_directory.map(pipeline_v3_recovery).map(Box::new);
    }
    failure
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

fn pipeline_v3_context(
    cpu_threads: u16,
    memory_mib: u64,
    storage_mib: u64,
    gpu_available: bool,
    network_available: bool,
    allow_side_effect: Vec<SideEffectArgument>,
    offline: bool,
) -> PipelinePlanningContext {
    PipelinePlanningContext {
        host: HostResources {
            cpu_threads,
            memory_mib,
            storage_mib,
            gpu_available,
            network_available,
        },
        allowed_side_effects: allow_side_effect.into_iter().map(Into::into).collect(),
        offline,
    }
}

fn load_provider_registry(paths: &[PathBuf]) -> Result<ProviderRegistry> {
    let mut registry = ProviderRegistry::new();
    for path in paths {
        let registration = ProviderRegistrationDocument::load(path)?.into_registration()?;
        registry.register(registration)?;
    }
    Ok(registry)
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
            if let Some(losses) = &failure.timed_text_losses {
                for loss in losses {
                    eprintln!(
                        "  {:?} {}: {}",
                        loss.kind,
                        escape_terminal_controls(&loss.field),
                        escape_terminal_controls(&loss.detail)
                    );
                }
            }
            if let Some(preflight) = &failure.audio_preflight {
                for diagnostic in &preflight.diagnostics {
                    eprintln!(
                        "  {:?} {}: {}",
                        diagnostic.code,
                        escape_terminal_controls(&diagnostic.tool),
                        escape_terminal_controls(&diagnostic.message)
                    );
                }
            }
            if let Some(preflight) = &failure.transcription_preflight {
                for diagnostic in &preflight.diagnostics {
                    eprintln!(
                        "  {:?} {}: {}",
                        diagnostic.code,
                        escape_terminal_controls(&diagnostic.component),
                        escape_terminal_controls(&diagnostic.message)
                    );
                }
            }
            if let Some(preflight) = &failure.alignment_preflight {
                for diagnostic in &preflight.diagnostics {
                    eprintln!(
                        "  {:?} {}: {}",
                        diagnostic.code,
                        escape_terminal_controls(&diagnostic.component),
                        escape_terminal_controls(&diagnostic.message)
                    );
                }
            }
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
            let rendered = if let Some(losses) = &failure.timed_text_losses {
                render_json(&MachineEnvelope::failure(
                    command,
                    &failure.error,
                    Some(serde_json::json!({ "losses": losses })),
                ))?
            } else if let Some(preflight) = &failure.transcription_preflight {
                render_json(&MachineEnvelope::failure(
                    command,
                    &failure.error,
                    Some(preflight.clone()),
                ))?
            } else if let Some(preflight) = &failure.alignment_preflight {
                render_json(&MachineEnvelope::failure(
                    command,
                    &failure.error,
                    Some(preflight.clone()),
                ))?
            } else if let Some(preflight) = &failure.audio_preflight {
                render_json(&MachineEnvelope::failure(
                    command,
                    &failure.error,
                    Some(preflight.clone()),
                ))?
            } else if let Some(planning) = &failure.planning {
                render_json(&MachineEnvelope::failure(
                    command,
                    &failure.error,
                    Some(planning.clone()),
                ))?
            } else if let Some(recovery) = &failure.pipeline_v3_recovery {
                render_json(&MachineEnvelope::failure(
                    command,
                    &failure.error,
                    Some(recovery.clone()),
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

fn observe_pipeline_v3_progress(
    presentation: Presentation,
    recovery_run_directory: &mut Option<PathBuf>,
    progress: &PipelineV3RunProgress,
) {
    if let PipelineV3RunProgress::Started { run_directory, .. } = progress {
        *recovery_run_directory = Some(run_directory.clone());
    }
    if presentation == Presentation::Human {
        print_pipeline_v3_progress(progress);
    }
}

fn pipeline_v3_recovery(run_directory: PathBuf) -> PipelineV3RunRecovery {
    let run_manifest = PipelineV3Workspace::open_read_only(&run_directory)
        .ok()
        .and_then(|workspace| {
            aniflow::status_v3(workspace.root())
                .ok()
                .map(|manifest| workspace.manifest_revision(manifest.payload.revision))
        });
    PipelineV3RunRecovery {
        schema: PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1.to_owned(),
        run_directory,
        run_manifest,
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

fn print_pipeline_v3_progress(progress: &PipelineV3RunProgress) {
    match progress {
        PipelineV3RunProgress::Started {
            operation,
            run_directory,
            pipeline_name,
        } => {
            let operation = format!("{operation:?}").to_lowercase();
            println!("aniflow {operation}-v3");
            println!();
            println!("  workspace    {}", run_directory.display());
            println!("  pipeline     {pipeline_name}");
            println!();
        }
        PipelineV3RunProgress::Stage { stage_id, state } => {
            println!("  {stage_id:<24} {}", pipeline_v3_progress_state(*state));
        }
    }
}

fn print_pipeline_v3_outcome(outcome: &PipelineV3RunOutcome) {
    println!();
    println!("complete");
    println!("  workspace    {}", outcome.run_directory.display());
    println!("  plan         sha256:{}", outcome.plan_sha256);
    println!("  manifest     {}", outcome.run_manifest.display());
    println!("  executed     {}", outcome.executed_stages.len());
    println!("  reused       {}", outcome.reused_stages.len());
    println!("  outputs");
    for output in &outcome.outputs {
        println!(
            "    {:<20} {} sha256:{}",
            output.id,
            output.path.display(),
            output.sha256
        );
    }
}

const fn pipeline_v3_progress_state(state: PipelineV3ProgressState) -> &'static str {
    match state {
        PipelineV3ProgressState::Running => "running",
        PipelineV3ProgressState::Validating => "validating",
        PipelineV3ProgressState::Complete => "complete",
        PipelineV3ProgressState::Reused => "reused",
        PipelineV3ProgressState::Invalidated => "invalidated",
        PipelineV3ProgressState::Failed => "failed",
        PipelineV3ProgressState::Cancelled => "cancelled",
    }
}

fn print_pipeline_v3_status(status: &PipelineV3RunManifest) {
    println!("aniflow status-v3");
    println!();
    println!("  run          {}", status.payload.run_id);
    println!("  plan         sha256:{}", status.payload.plan_sha256);
    println!("  revision     {}", status.payload.revision);
    println!(
        "  state        {}",
        format!("{:?}", status.payload.state).to_lowercase()
    );
    println!();
    println!("stages");
    for stage in &status.payload.stages {
        println!(
            "  {:<24} {}",
            stage.stage_id,
            pipeline_v3_stage_state(stage.state)
        );
        if let Some(message) = &stage.message {
            println!("    {message}");
        }
    }
    println!();
    println!("outputs");
    for output in &status.payload.outputs {
        let path = output.relative_path.as_deref().unwrap_or("external input");
        println!("  {:<24} {path} sha256:{}", output.id, output.sha256);
    }
}

const fn pipeline_v3_stage_state(state: PipelineV3StageState) -> &'static str {
    match state {
        PipelineV3StageState::Pending => "pending",
        PipelineV3StageState::Running => "running",
        PipelineV3StageState::Validating => "validating",
        PipelineV3StageState::Complete => "complete",
        PipelineV3StageState::Failed => "failed",
        PipelineV3StageState::Cancelled => "cancelled",
        PipelineV3StageState::Invalidated => "invalidated",
        PipelineV3StageState::Skipped => "skipped",
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
