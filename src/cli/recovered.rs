use super::*;
use aniflow::music_video_recipe::recovered::{self,RecoveredRequest};
#[derive(Debug,Subcommand)]
pub(super) enum RecoveredCommands {
    WithRife {#[command(subcommand)] command:recovered_recipe_v2::RecoveredV2Commands},
    Plan {#[arg(long)] request:PathBuf},Run {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Resume {#[arg(long)] run_directory:PathBuf},Status {#[arg(long)] run_directory:PathBuf},
}
impl RecoveredCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::WithRife {command}=>command.name(),
    Self::Plan {..}=>CommandName::VideoRecoveredPlan,Self::Run {..}=>CommandName::VideoRecoveredRun,
    Self::Resume {..}=>CommandName::VideoRecoveredResume,Self::Status {..}=>CommandName::VideoRecoveredStatus,
}}}
fn read(path:&Path)->Result<RecoveredRequest> {RecoveredRequest::from_json_slice(&read_bounded_configuration(path,"recovered music-video request",4_194_304)?)}
pub(super) fn dispatch(command:RecoveredCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    let value=match command {
        RecoveredCommands::WithRife {command}=>return recovered_recipe_v2::dispatch(command,presentation),
        RecoveredCommands::Plan {request}=>{let value=recovered::plan(&read(&request)?,&token)?;return print_result(name,presentation,&value,||println!("Accepted selected frames: {}; missing frames: {}; bounded native runs: {}",value.intent.initial_selection.frames.len(),value.intent.missing_inputs.len(),value.intent.batches.len()));},
        RecoveredCommands::Run {request,run_directory}=>{let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());recovered::run(read(&request)?,&root,&token,|_,event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?},
        RecoveredCommands::Resume {run_directory}=>{let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());recovered::resume(&root,&token,|_,event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?},
        RecoveredCommands::Status {run_directory}=>recovered::status(&absolute_delivery_path(run_directory)?,&token)?,
    };
    print_result(name,presentation,&value,||{println!("Reused selection: {}; original missing work: {}; complete: {}",value.reused_frames,value.missing_frames,value.complete);println!("Creative/publication review remains separate");})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_entry_requires_request_and_root_while_resume_replays_the_immutable_intent() {
        assert!(Cli::try_parse_from(["aniflow","video","recovered","run","--request","recovered.json"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","video","recovered","run","--request","recovered.json","--run-directory","private/run"]).is_ok());
        assert!(Cli::try_parse_from(["aniflow","video","recovered","resume","--run-directory","private/run","--request","changed.json"]).is_err());
    }
}
