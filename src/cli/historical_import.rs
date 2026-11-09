//! Thin selected historical-frame import delivery.
use super::*;
use aniflow::upscayl_recovery::historical::{self,ImportRequest};
#[derive(Debug,Subcommand)]
pub(super) enum HistoricalImportCommands {
    ReviewPlan {#[arg(long)] request:PathBuf},
    Plan {#[arg(long)] request:PathBuf,#[arg(long)] declaration:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] declaration:PathBuf,#[arg(long)] output_directory:PathBuf},
    Resume {#[arg(long)] request:PathBuf,#[arg(long)] declaration:PathBuf,#[arg(long)] run_directory:PathBuf},
    Status {#[arg(long)] run_directory:PathBuf},
}
impl HistoricalImportCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::ReviewPlan {..}=>CommandName::VideoHistoricalImportReviewPlan,Self::Plan {..}=>CommandName::VideoHistoricalImportPlan,
    Self::Run {..}=>CommandName::VideoHistoricalImportRun,Self::Resume {..}=>CommandName::VideoHistoricalImportResume,Self::Status {..}=>CommandName::VideoHistoricalImportStatus,
}}}
fn read(path:&Path)->Result<ImportRequest> {serde_json::from_slice(&read_bounded_configuration(path,"historical import request",262_144)?).map_err(|_|Error::new(ErrorCategory::Configuration,"invalid closed historical import request"))}
fn decision(path:&Path)->Result<aniflow::music_video_recipe::manual::ManualReview> {serde_json::from_slice(&read_audio_configuration(path,"historical import decision")?).map_err(|_|Error::new(ErrorCategory::Configuration,"invalid supplied import decision"))}
pub(super) fn dispatch(command:HistoricalImportCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
        HistoricalImportCommands::ReviewPlan {request}=>{let value=historical::review_plan(&read(&request)?,&token)?;print_result(name,presentation,&value,||{println!("Historical import context: {}",value.context_sha256);println!("Selected subject: {}",value.subject_sha256);println!("Selected frames: {}; prior history unknown; no import decision inferred",value.intent.frames.len());})},
        HistoricalImportCommands::Plan {request,declaration}=>{let value=historical::plan(&read(&request)?,&decision(&declaration)?,&token)?;print_result(name,presentation,&value,||println!("Historical native import plan: {}",value.plan_sha256))},
        HistoricalImportCommands::Run {request,declaration,output_directory}=>{let mut recovery=None;let value=historical::run(read(&request)?,decision(&declaration)?,&absolute_delivery_path(output_directory)?,&token,|e|observe_pipeline_v3_progress(presentation,&mut recovery,e))?;print_result(name,presentation,&value,||println!("Historical import native stages executed: {}",value.executed_stages.len()))},
        HistoricalImportCommands::Resume {request,declaration,run_directory}=>{let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());let value=historical::resume(read(&request)?,decision(&declaration)?,&root,&token,|e|observe_pipeline_v3_progress(presentation,&mut recovery,e))?;print_result(name,presentation,&value,||println!("Historical import native stages executed: {}",value.executed_stages.len()))},
        HistoricalImportCommands::Status {run_directory}=>{let value=historical::observe(&absolute_delivery_path(run_directory)?,&token)?;print_result(name,presentation,&value,||println!("Accepted historical selection: {} frames; original history remains unknown",value.subset.frames.len()))},
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_requires_supplied_decision_and_never_adopts_partial_outputs() {
        let args=["aniflow","video","historical-import","plan","--request","private frames.json"];
        assert!(Cli::try_parse_from(args).is_err());assert_eq!(Cli::try_parse_from(args.into_iter().chain(["--declaration","decision.json"])).unwrap().command.name(),CommandName::VideoHistoricalImportPlan);
        for flag in ["--adopt","--approve","--overwrite","--guess-model"] {assert!(Cli::try_parse_from(args.into_iter().chain(["--declaration","decision.json",flag])).is_err());}
    }
}
