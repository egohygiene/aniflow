//! Thin command delivery for the fixed accepted-video finishing tail.
use super::*;
use aniflow::music_video_recipe::finishing::{self,FinishingRequest,FinishingStatus};
#[derive(Debug,Subcommand)]
pub(super) enum FinishingCommands {
    Plan {#[arg(long)] request:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Resume {#[arg(long)] run_directory:PathBuf},
    Status {#[arg(long)] run_directory:PathBuf},
    RecordReview {#[arg(long)] run_directory:PathBuf,#[arg(long)] declaration:PathBuf},
}
impl FinishingCommands {
    pub(super) fn name(&self)->CommandName {match self {Self::Plan {..}=>CommandName::VideoFinishingPlan,Self::Run {..}=>CommandName::VideoFinishingRun,Self::Resume {..}=>CommandName::VideoFinishingResume,Self::Status {..}=>CommandName::VideoFinishingStatus,Self::RecordReview {..}=>CommandName::VideoFinishingRecordReview}}
}
fn read(path:&Path)->Result<FinishingRequest> {FinishingRequest::from_json_slice(&read_audio_configuration(path,"finishing request")?)}
fn print_status(value:&FinishingStatus) {
    println!("Finishing complete: {}",value.complete);
    println!("  run: {}",escape_terminal_controls(&value.run_directory.display().to_string()));
    for step in &value.steps {println!("  {:?}: {:?}",step.phase,step.state);}
    if let Some(preview)=&value.preview {println!("  preview context: {}",preview.binding.context_sha256);println!("  preview subject: {}",preview.binding.subject_sha256);println!("  preview frames: {}",escape_terminal_controls(&preview.frames_directory.display().to_string()));}
    println!("Full delivery creative review and manual publishing remain separate");
}
pub(super) fn dispatch(command:FinishingCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    let value=match command {
        FinishingCommands::Plan {request}=>{let value=finishing::plan(&read(&request)?,&token)?;return print_result(name,presentation,&value,||{for step in &value.steps {println!("{:?}: {}",step.phase,if step.plan.is_some(){"planned"}else{"pending accepted predecessor/review"});}});},
        FinishingCommands::Run {request,run_directory}=>{
            let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            finishing::run(read(&request)?,&root,&token,|_,event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?
        },
        FinishingCommands::Resume {run_directory}=>{
            let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            finishing::resume(&root,&token,|_,event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?
        },
        FinishingCommands::Status {run_directory}=>finishing::status(&absolute_delivery_path(run_directory)?,&token)?,
        FinishingCommands::RecordReview {run_directory,declaration}=>{
            let declaration=serde_json::from_slice(&read_audio_configuration(&declaration,"caption preview declaration")?).map_err(|_|Error::new(ErrorCategory::Configuration,"invalid supplied caption preview declaration"))?;
            finishing::record_review(&absolute_delivery_path(run_directory)?,&declaration,&token)?
        },
    };
    print_result(name,presentation,&value,||print_status(&value))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_review_and_resume_are_separate_without_implicit_approval_flags() {
        let base=["aniflow","video","finishing","record-review","--run-directory","private recipe"];
        assert!(Cli::try_parse_from(base).is_err());assert_eq!(Cli::try_parse_from(base.into_iter().chain(["--declaration","decision.json"])).unwrap().command.name(),CommandName::VideoFinishingRecordReview);
        for flag in ["--approve","--reject","--resume","--force"] {assert!(Cli::try_parse_from(base.into_iter().chain(["--declaration","decision.json",flag])).is_err());}
        assert_eq!(Cli::try_parse_from(["aniflow","video","finishing","resume","--run-directory","recipe"]).unwrap().command.name(),CommandName::VideoFinishingResume);
    }
}
