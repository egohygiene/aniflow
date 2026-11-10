//! Fixed runtime delivery; no second interpolation implementation or journal.
use super::*;
use aniflow::video_interpolation::{RifeConfiguration,runtime};
use aniflow::video_interpolation::preview::{self,PreviewRequest,PreviewRange};
#[derive(Debug,Clone,Args)]
pub(super) struct RifeArguments {
    #[arg(long)] frames_directory:PathBuf,
    #[arg(long)] frame_index:PathBuf,
    /// Closed configuration with exact local pins and finite resource bounds.
    #[arg(long)] configuration:PathBuf,
}
#[derive(Debug,Clone,Args)]
pub(super) struct PreviewArguments {
    #[command(flatten)] arguments:RifeArguments,
    #[arg(long)] first_source_ordinal:u64,
    /// Exclusive original-source end, at most 120 frames / ten seconds.
    #[arg(long)] end_source_ordinal:u64,
}
impl PreviewArguments {
    fn request(self)->Result<PreviewRequest> {Ok(PreviewRequest::new(self.arguments.request()?,PreviewRange {first_source_ordinal:self.first_source_ordinal,end_source_ordinal:self.end_source_ordinal}))}
}
impl RifeArguments {
    fn request(self)->Result<runtime::RifeRunRequest> {
        let configuration=RifeConfiguration::from_json_slice(&read_audio_configuration(&self.configuration,"RIFE configuration")?)?;
        let provider=std::env::current_exe().map_err(|_|Error::new(ErrorCategory::Dependency,"cannot identify the native aniflow provider executable"))?;
        Ok(runtime::RifeRunRequest::new(absolute_delivery_path(self.frames_directory)?,absolute_delivery_path(self.frame_index)?,configuration,provider))
    }
}
#[derive(Debug,Subcommand)]
pub(super) enum RifeCommands {
    Recovered {#[command(subcommand)] command:recovered_rife::RecoveredRifeCommands},
    PreviewMovie {#[command(subcommand)] command:rife_movie::MovieCommands},
    PreviewPlan {#[command(flatten)] arguments:PreviewArguments},
    PreviewRun {#[command(flatten)] arguments:PreviewArguments,#[arg(long)] output_directory:PathBuf},
    PreviewResume {#[command(flatten)] arguments:PreviewArguments,#[arg(long)] run_directory:PathBuf},
    /// Read current accepted preview PNGs/mapping; use status-v3 for partial work.
    PreviewStatus {#[arg(long)] run_directory:PathBuf},
    /// Recheck actual PNG/index/tool/model bytes and bounds without launching tools.
    Plan {#[command(flatten)] arguments:RifeArguments},
    /// Start one isolated ordinary v3 serial interpolation run.
    Run {#[command(flatten)] arguments:RifeArguments,#[arg(long)] output_directory:PathBuf},
    /// Resume the exact run with unchanged inputs, pins and settings.
    Resume {#[command(flatten)] arguments:RifeArguments,#[arg(long)] run_directory:PathBuf},
    /// Read fixed-profile native state without processes or repair.
    Status {#[arg(long)] run_directory:PathBuf},
}
impl RifeCommands {
    pub(super) fn name(&self)->CommandName {match self {
        Self::Recovered {command}=>command.name(),
        Self::PreviewMovie {command}=>command.name(),
        Self::PreviewPlan {..}=>CommandName::VideoRifePreviewPlan,Self::PreviewRun {..}=>CommandName::VideoRifePreviewRun,
        Self::PreviewResume {..}=>CommandName::VideoRifePreviewResume,Self::PreviewStatus {..}=>CommandName::VideoRifePreviewStatus,
        Self::Plan {..}=>CommandName::VideoRifePlan,Self::Run {..}=>CommandName::VideoRifeRun,
        Self::Resume {..}=>CommandName::VideoRifeResume,Self::Status {..}=>CommandName::VideoRifeStatus,
    }}
}
pub(super) fn dispatch(command:RifeCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();
    match command {
        RifeCommands::Recovered {command}=>recovered_rife::dispatch(command,presentation),
        RifeCommands::PreviewMovie {command}=>rife_movie::dispatch(command,presentation),
        RifeCommands::PreviewPlan {arguments}=>{
            let result=preview::plan(&arguments.request()?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||print_pipeline_v3_plan(&result))
        },
        RifeCommands::PreviewRun {arguments,output_directory}=>{
            let request=arguments.request()?;let root=absolute_delivery_path(output_directory)?;let token=cli_cancellation_token()?;let mut recovery=None;
            let result=preview::run(request,&root,&token,|event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,recovery))?;
            print_result(name,presentation,&result,||print_pipeline_v3_outcome(&result))
        },
        RifeCommands::PreviewResume {arguments,run_directory}=>{
            let root=absolute_delivery_path(run_directory)?;let request=arguments.request().map_err(|e|CommandFailure::pipeline_v3(e,Some(root.clone())))?;
            let token=cli_cancellation_token().map_err(|e|CommandFailure::pipeline_v3(e,Some(root.clone())))?;let mut recovery=Some(root.clone());
            let result=preview::resume(request,&root,&token,|event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,recovery))?;
            print_result(name,presentation,&result,||print_pipeline_v3_outcome(&result))
        },
        RifeCommands::PreviewStatus {run_directory}=>{
            let result=preview::observe(&absolute_delivery_path(run_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("{} preview PNGs on the original clock; no full-run or perceptual acceptance",result.report.frames.len()))
        },
        RifeCommands::Plan {arguments}=>{
            let result=runtime::plan(&arguments.request()?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||print_pipeline_v3_plan(&result))
        },
        RifeCommands::Run {arguments,output_directory}=>{
            let request=arguments.request()?;let parent=absolute_delivery_path(output_directory)?;let token=cli_cancellation_token()?;
            let mut recovery=None;
            let progress=|event:&PipelineV3RunProgress|observe_pipeline_v3_progress(presentation,&mut recovery,event);
            let result=runtime::run(request,Some(parent),&token,progress).map_err(|error|CommandFailure::pipeline_v3(error,recovery))?;
            print_result(name,presentation,&result,||print_pipeline_v3_outcome(&result))
        },
        RifeCommands::Resume {arguments,run_directory}=>{
            let root=absolute_delivery_path(run_directory)?;
            let request=arguments.request().map_err(|error|CommandFailure::pipeline_v3(error,Some(root.clone())))?;
            let token=cli_cancellation_token().map_err(|error|CommandFailure::pipeline_v3(error,Some(root.clone())))?;
            let mut recovery=Some(root.clone());
            let progress=|event:&PipelineV3RunProgress|observe_pipeline_v3_progress(presentation,&mut recovery,event);
            let result=runtime::resume(request,&root,&token,progress).map_err(|error|CommandFailure::pipeline_v3(error,recovery))?;
            print_result(name,presentation,&result,||print_pipeline_v3_outcome(&result))
        },
        RifeCommands::Status {run_directory}=>{
            let result=runtime::status(absolute_delivery_path(run_directory)?)?;
            print_result(name,presentation,&result,||print_pipeline_v3_status(&result))
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_requires_an_explicit_window_and_keeps_full_run_commands_separate() {
        let base=["aniflow","video","rife","preview-plan","--frames-directory","frames","--frame-index","index.json","--configuration","rife.json"];
        assert!(Cli::try_parse_from(base).is_err());
        assert_eq!(Cli::try_parse_from(base.into_iter().chain(["--first-source-ordinal","30","--end-source-ordinal","90"])).unwrap().command.name(),CommandName::VideoRifePreviewPlan);
        assert!(Cli::try_parse_from(base.into_iter().chain(["--first-source-ordinal","30","--end-source-ordinal","90","--approve-full-video"])).is_err());
    }
    #[test]
    fn refused_preview_resume_retains_the_explicit_recovery_root() {
        let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();
        let arguments=PreviewArguments {arguments:RifeArguments {frames_directory:root.join("frames"),frame_index:root.join("index.json"),configuration:root.join("missing.json")},first_source_ordinal:0,end_source_ordinal:1};
        let failure=dispatch(RifeCommands::PreviewResume {arguments,run_directory:root.clone()},Presentation::Machine).unwrap_err();
        assert_eq!(failure.pipeline_v3_recovery.unwrap().run_directory,root);assert_eq!(std::fs::read_dir(root).unwrap().count(),0);
    }
    #[test]
    fn rife_modes_require_indexed_inputs_and_separate_destinations() {
        for (mode,tail,name) in [("plan",vec![],CommandName::VideoRifePlan),("run",vec!["--output-directory","new runs"],CommandName::VideoRifeRun),("resume",vec!["--run-directory","existing run"],CommandName::VideoRifeResume)] {
            let base=["aniflow","--output","json","video","rife",mode,"--frames-directory","frames 音","--frame-index","index.json","--configuration","rife.json"];
            assert_eq!(Cli::try_parse_from(base.into_iter().chain(tail.clone())).unwrap().command.name(),name);
            assert!(Cli::try_parse_from(base.into_iter().chain(tail).chain(["--download"])).is_err());
        }
        for mode in ["run","resume"] {assert!(Cli::try_parse_from(["aniflow","video","rife",mode,"--frames-directory","frames","--frame-index","index.json","--configuration","rife.json"]).is_err());}
    }
    #[test]
    fn rife_status_has_no_execution_or_model_selection_effects() {
        let base=["aniflow","video","rife","status","--run-directory","existing run"];
        assert_eq!(Cli::try_parse_from(base).unwrap().command.name(),CommandName::VideoRifeStatus);
        for flag in ["--execute","--force","--install","--download","--auto-model"] {assert!(Cli::try_parse_from(base.into_iter().chain([flag])).is_err());}
    }
    #[test]
    fn loose_status_refuses_without_creating_runtime_authority() {
        let directory=tempfile::tempdir().unwrap();let root=directory.path().canonicalize().unwrap();
        std::fs::write(root.join("output-index.json"),b"unaccepted loose bytes").unwrap();
        assert!(dispatch(RifeCommands::Status {run_directory:root.clone()},Presentation::Machine).is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(),1);assert!(!root.join("writer.lock").exists());
    }
    #[test]
    fn refused_resume_keeps_named_root_without_inventing_manifest() {
        let directory=tempfile::tempdir().unwrap();let root=directory.path().canonicalize().unwrap();
        let arguments=RifeArguments {frames_directory:root.join("frames"),frame_index:root.join("index.json"),configuration:root.join("missing.json")};
        let failure=dispatch(RifeCommands::Resume {arguments,run_directory:root.clone()},Presentation::Machine).unwrap_err();
        let recovery=failure.pipeline_v3_recovery.unwrap();assert_eq!(recovery.run_directory,root);assert!(recovery.run_manifest.is_none());assert_eq!(std::fs::read_dir(root).unwrap().count(),0);
    }
}
