//! Thin bounded configuration delivery; native work stays in existing owners.
use super::*;
use aniflow::music_video_recipe::stages::{self,Request,manual::Change,entry};
#[derive(Debug,Subcommand)]
pub(super) enum StageCommands {
    Plan {#[arg(long)] request:PathBuf},
    Status {#[arg(long)] request:PathBuf},
    Manual {#[arg(long)] request:PathBuf,#[arg(long)] change:PathBuf},
    EntryStatus {#[arg(long)] request:PathBuf},
    EntryReview {#[arg(long)] request:PathBuf,#[arg(long)] decision:PathBuf},
}
impl StageCommands {pub(super) fn name(&self)->CommandName {match self {Self::Plan {..}=>CommandName::VideoStagesPlan,Self::Status {..}=>CommandName::VideoStagesStatus,Self::Manual {..}=>CommandName::VideoStagesManual,Self::EntryStatus {..}=>CommandName::VideoStagesEntryStatus,Self::EntryReview {..}=>CommandName::VideoStagesEntryReview}}}
fn read(path:&Path)->Result<Request>{Request::from_json_slice(&read_bounded_configuration(path,"fixed release-stage request",262_144)?)}
pub(super) fn dispatch(command:StageCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
        StageCommands::EntryStatus {request}=>{
            let request=entry::Request::from_json_slice(&read_bounded_configuration(&request,"source-entry stage request",524_288)?)?;let value=entry::observe(&request,&token)?;
            print_result(name,presentation,&value,||println!("Source-entry journey: {:?}; next {:?}; no native work started",value.state,value.next_action))
        },
        StageCommands::EntryReview {request,decision}=>{
            let request=entry::Request::from_json_slice(&read_bounded_configuration(&request,"source-entry stage request",524_288)?)?;let decision=entry::Decision::from_json_slice(&read_bounded_configuration(&decision,"supplied source-entry decision",65_536)?)?;let value=entry::record(&request,&decision,&token)?;
            print_result(name,presentation,&value,||println!("Retained supplied exact prepared-frame decision; next {:?}; assembly resumes separately",value.next_action))
        },
        StageCommands::Plan {request}|StageCommands::Status {request}=>{
            let value=stages::observe(&read(&request)?,&token)?;
            print_result(name,presentation,&value,||{for step in &value.steps{println!("{:?}: {:?} ({:?})",step.stage,step.state,step.handoff.action);}println!("No work started; existing native/review owners remain authoritative");})
        },
        StageCommands::Manual {request,change}=>{
            let change=Change::from_json_slice(&read_bounded_configuration(&change,"supplied manual stage change",32_768)?)?;
            let value=stages::manual::apply(&read(&request)?,&change,&token)?;
            print_result(name,presentation,&value,||println!("Retained explicit manual change {}; no native execution or publication",value.change_sha256))
        },
    }
}
#[cfg(test)] mod tests {use super::*;
#[test] fn stage_views_do_not_accept_execution_or_approval_flags(){for verb in ["plan","status"]{let args=["aniflow","video","stages",verb,"--request","journey.json"];assert!(Cli::try_parse_from(args).is_ok());for flag in ["--execute","--approve","--download","--post"]{assert!(Cli::try_parse_from(args.into_iter().chain([flag])).is_err());}}}
#[test] fn manual_stage_action_requires_a_supplied_change_document(){assert!(Cli::try_parse_from(["aniflow","video","stages","manual","--request","journey.json"]).is_err());assert!(Cli::try_parse_from(["aniflow","video","stages","manual","--request","journey.json","--change","operator-change.json"]).is_ok());}
#[test] fn entry_view_and_decision_keep_native_resume_and_default_approval_out(){assert!(Cli::try_parse_from(["aniflow","video","stages","entry-status","--request","entry.json"]).is_ok());assert!(Cli::try_parse_from(["aniflow","video","stages","entry-status","--request","entry.json","--resume"]).is_err());assert!(Cli::try_parse_from(["aniflow","video","stages","entry-review","--request","entry.json"]).is_err());assert!(Cli::try_parse_from(["aniflow","video","stages","entry-review","--request","entry.json","--decision","supplied.json"]).is_ok());}
}
