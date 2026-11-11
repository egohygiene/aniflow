//! Delivery only: manual lyrics and recipe attachment stay library-owned.
use super::*;
use aniflow::music_video_recipe::{lyrics, manual::ManualReview, recipe_lyrics};

#[derive(Debug, Clone, Args)]
pub(super) struct WorkflowArguments {
    /// Exact saved aniflow.manual-lyrics-request/v1 JSON, at most 64 KiB.
    #[arg(long)]
    request: PathBuf,
    /// Separate private manual work area; existing parent required on initialize.
    #[arg(long)]
    work_directory: PathBuf,
}
impl WorkflowArguments {
    fn resolve(self) -> Result<(lyrics::ManualLyricsRequest, PathBuf)> {
        Ok((read_delivery_document(&self.request, "manual lyrics request")?, absolute_delivery_path(self.work_directory)?))
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum ReviewScopeArgument { Words, Timing }

#[derive(Debug, Subcommand)]
pub(super) enum LyricsCommands {
    /// Private current cue worksheet; supplies no words/timing approval.
    Worksheet {#[command(flatten)] workflow:WorkflowArguments},
    RevisePlan {#[arg(long)] request:PathBuf},
    ReviseCreate {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    RevisionStatus {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
    /// Return a request bound to completed recipe audio; writes no workflow.
    PrepareRequest {
        #[arg(long)]
        recipe_run_directory: PathBuf,
        /// Explicit RecipeLyricsOptions JSON (format, language, overlap, targets, conversion).
        #[arg(long)]
        options: PathBuf,
    },
    /// Reserve a fresh manual work area, optionally importing supplied intervals.
    Initialize {
        #[command(flatten)]
        workflow: WorkflowArguments,
        #[arg(long)]
        input: Option<PathBuf>,
    },
    /// Attach an explicitly authored SRT, WebVTT or TTML; supplies no review.
    Import {
        #[command(flatten)]
        workflow: WorkflowArguments,
        #[arg(long)]
        input: PathBuf,
    },
    /// Read current words/timing fingerprints and review states without writing.
    Status {
        #[command(flatten)]
        workflow: WorkflowArguments,
    },
    /// Retain an existing exact operator declaration for one scope, once.
    Review {
        #[command(flatten)]
        workflow: WorkflowArguments,
        #[arg(long, value_enum)]
        scope: ReviewScopeArgument,
        /// Explicit aniflow.manual-review/v1 JSON; no decision or hash is filled in.
        #[arg(long)]
        declaration: PathBuf,
    },
    /// Publish selected sidecars into a fresh directory after both reviews.
    Export {
        #[command(flatten)]
        workflow: WorkflowArguments,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Attach reviewed sidecars to accepted recipe assembly; never rewrites video.
    Attach {
        #[command(flatten)]
        workflow: WorkflowArguments,
        #[arg(long)]
        recipe_run_directory: PathBuf,
    },
}
impl LyricsCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Worksheet {..}=>CommandName::VideoLyricsWorksheet,
            Self::RevisePlan {..}=>CommandName::VideoLyricsRevisePlan,Self::ReviseCreate {..}=>CommandName::VideoLyricsReviseCreate,
            Self::RevisionStatus {..}=>CommandName::VideoLyricsRevisionStatus,
            Self::PrepareRequest { .. } => CommandName::VideoLyricsPrepareRequest,
            Self::Initialize { .. } => CommandName::VideoLyricsInitialize,
            Self::Import { .. } => CommandName::VideoLyricsImport,
            Self::Status { .. } => CommandName::VideoLyricsStatus,
            Self::Review { .. } => CommandName::VideoLyricsReview,
            Self::Export { .. } => CommandName::VideoLyricsExport,
            Self::Attach { .. } => CommandName::VideoLyricsAttach,
        }
    }
}
fn print_status(status: &lyrics::ManualLyricsStatus) {
    for (scope, current) in [("words", &status.words), ("timing", &status.timing)] {
        println!("{scope}: {:?}", current.state);
        println!("  context_sha256: {}", current.context_sha256);
        println!("  subject_sha256: {}", current.subject_sha256.as_deref().unwrap_or("awaiting input"));
    }
    println!("Ready for sidecar export: {}", status.ready_for_sidecar_export);
    println!("Render review: not performed");
}
pub(super) fn dispatch(command: LyricsCommands, presentation: Presentation) -> CommandResult<()> {
    let name = command.name();
    match command {
        LyricsCommands::Worksheet {workflow}=>{
            let (request,root)=workflow.resolve()?;let value=lyrics::worksheet::observe(&request,&root,&cli_cancellation_token()?)?;
            print_result(name,presentation,&value,||println!("Private cue worksheet {}; {} cues; no review approval supplied",value.worksheet_sha256,value.payload.audio_relative.cues.len()))
        },
        LyricsCommands::RevisePlan {request}=>{
            let request=lyrics::revision::RevisionRequest::from_json_slice(&read_bounded_configuration(&request,"manual lyric revision request",131_072)?)?;
            let value=lyrics::revision::plan(&request,&cli_cancellation_token()?)?;
            print_result(name,presentation,&value,||println!("Fresh revision intent {}; {} text / {} timing changes by cue ID; both reviews start fresh",value.intent_sha256,value.changes.text_changed,value.changes.timing_changed))
        },
        LyricsCommands::ReviseCreate {request,output_directory}=>{
            let request=lyrics::revision::RevisionRequest::from_json_slice(&read_bounded_configuration(&request,"manual lyric revision request",131_072)?)?;
            let value=lyrics::revision::create(&request,&absolute_delivery_path(output_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&value,||{println!("Fresh revision receipt {}; new workflow is under lyrics/",value.receipt_sha256);print_status(&value.current);})
        },
        LyricsCommands::RevisionStatus {directory,receipt_sha256}=>{
            let value=lyrics::revision::observe(&absolute_delivery_path(directory)?,&receipt_sha256,&cli_cancellation_token()?)?;
            print_result(name,presentation,&value,||print_status(&value.current))
        },
        LyricsCommands::PrepareRequest { recipe_run_directory, options } => {
            let result = recipe_lyrics::request(absolute_delivery_path(recipe_run_directory)?,
                read_delivery_document(&options, "recipe lyrics options")?)?;
            let human = serde_json::to_string_pretty(&result)
                .map_err(|_| Error::new(ErrorCategory::Internal, "cannot serialize manual lyrics request"))?;
            print_result(name, presentation, &result, || {
                // Explicit private request output, also usable without machine envelopes.
                println!("{human}");
            })
        }
        LyricsCommands::Initialize { workflow, input } => {
            let (request, root) = workflow.resolve()?;
            let input = input.map(absolute_delivery_path).transpose()?;
            let result = lyrics::initialize(&request, &root, input.as_deref(), &cli_cancellation_token()?)?;
            print_result(name, presentation, &result, || print_status(&result))
        }
        LyricsCommands::Import { workflow, input } => {
            let (request, root) = workflow.resolve()?;
            let result = lyrics::import(&request, &root, &absolute_delivery_path(input)?, &cli_cancellation_token()?)?;
            print_result(name, presentation, &result, || print_status(&result))
        }
        LyricsCommands::Status { workflow } => {
            let (request, root) = workflow.resolve()?;
            let result = lyrics::status(&request, &root)?;
            print_result(name, presentation, &result, || print_status(&result))
        }
        LyricsCommands::Review { workflow, scope, declaration } => {
            let (request, root) = workflow.resolve()?;
            let declaration: ManualReview = read_delivery_document(&declaration, "manual review declaration")?;
            let scope = match scope { ReviewScopeArgument::Words => lyrics::LyricsReviewScope::Words, ReviewScopeArgument::Timing => lyrics::LyricsReviewScope::Timing };
            let result = lyrics::review(&request, &root, scope, &declaration)?;
            print_result(name, presentation, &result, || print_status(&result))
        }
        LyricsCommands::Export { workflow, output_directory } => {
            let (request, root) = workflow.resolve()?;
            let result = lyrics::export(&request, &root, &absolute_delivery_path(output_directory)?)?;
            print_result(name, presentation, &result, || println!("Exported {} selected sidecars; render review not performed", result.exports.len()))
        }
        LyricsCommands::Attach { workflow, recipe_run_directory } => {
            let (request, root) = workflow.resolve()?;
            let result = recipe_lyrics::attach(absolute_delivery_path(recipe_run_directory)?, &request, &root)?;
            print_result(name, presentation, &result, || println!("Attached {} reviewed sidecars; video unchanged", result.delivery.exports.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn revisions_require_explicit_parent_request_fresh_output_and_external_pin(){let args=["aniflow","video","lyrics","revise-create","--request","revision.json","--output-directory","fresh revision"];assert_eq!(Cli::try_parse_from(args).unwrap().command.name(),CommandName::VideoLyricsReviseCreate);assert!(Cli::try_parse_from(["aniflow","video","lyrics","revise-create","--request","revision.json"]).is_err());assert!(Cli::try_parse_from(["aniflow","video","lyrics","revision-status","--directory","revision"]).is_err());for flag in ["--approve","--transfer-review","--transcribe","--align","--overwrite"]{assert!(Cli::try_parse_from(args.into_iter().chain([flag])).is_err());}}
    #[test]
    fn manual_delivery_parser_keeps_scope_and_declaration_explicit() {
        let base = ["aniflow", "--output", "json", "video", "lyrics", "review", "--request", "request with spaces.json", "--work-directory", "private work"];
        for scope in ["words", "timing"] {
            let parsed = Cli::try_parse_from(base.into_iter().chain(["--scope", scope, "--declaration", "operator decision.json"])).unwrap();
            assert_eq!(parsed.command.name(), CommandName::VideoLyricsReview);
        }
        for tail in [vec!["--scope", "words"], vec!["--declaration", "decision.json"], vec!["--scope", "all", "--declaration", "decision.json"], vec!["--scope", "words", "--declaration", "decision.json", "--approve"]] {
            assert!(Cli::try_parse_from(base.into_iter().chain(tail)).is_err());
        }
    }
    #[test]
    fn manual_commands_select_distinct_envelopes_and_refuse_recognition() {
        for (verb, tail, name) in [
            ("initialize", vec![], CommandName::VideoLyricsInitialize),
            ("import", vec!["--input", "artist lyrics.srt"], CommandName::VideoLyricsImport),
            ("status", vec![], CommandName::VideoLyricsStatus),
            ("export", vec!["--output-directory", "new sidecars"], CommandName::VideoLyricsExport),
            ("attach", vec!["--recipe-run-directory", "accepted recipe"], CommandName::VideoLyricsAttach),
        ] {
            let prefix = ["aniflow", "video", "lyrics", verb, "--request", "request.json", "--work-directory", "manual work"];
            let parsed = Cli::try_parse_from(prefix.into_iter().chain(tail.clone())).unwrap();
            assert_eq!(parsed.command.name(), name);
            assert!(Cli::try_parse_from(prefix.into_iter().chain(tail).chain(["--auto-detect"])).is_err());
        }
        let parsed = Cli::try_parse_from(["aniflow", "video", "lyrics", "prepare-request", "--recipe-run-directory", "recipe", "--options", "options.json"]).unwrap();
        assert_eq!(parsed.command.name(), CommandName::VideoLyricsPrepareRequest);
    }
}
