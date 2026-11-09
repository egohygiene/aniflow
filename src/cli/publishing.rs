//! Thin delivery of the fixed publishing continuation and supplied decisions.
use super::*;
use aniflow::music_video_recipe::publishing::{self,PublishingRequest,PublishingStatus};
#[derive(Debug,Subcommand)]
pub(super) enum PublishingCommands {
    Plan {#[arg(long)] request:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Resume {#[arg(long)] run_directory:PathBuf},
    Status {#[arg(long)] run_directory:PathBuf},
    NextAction {#[arg(long)] run_directory:PathBuf},
    RecordReview {#[arg(long)] run_directory:PathBuf,#[arg(long)] declaration:PathBuf},
}
impl PublishingCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Plan {..}=>CommandName::DeliveryPublishingPlan,Self::Run {..}=>CommandName::DeliveryPublishingRun,
    Self::Resume {..}=>CommandName::DeliveryPublishingResume,Self::Status {..}=>CommandName::DeliveryPublishingStatus,
    Self::NextAction {..}=>CommandName::DeliveryPublishingNextAction,Self::RecordReview {..}=>CommandName::DeliveryPublishingRecordReview,
}}}
fn read(path:&Path)->Result<PublishingRequest> {PublishingRequest::from_json_slice(&read_audio_configuration(path,"fixed publishing request")?)}
fn human(value:&PublishingStatus) {
    for step in &value.steps {println!("{:?}: {:?}",step.phase,step.state);}
    println!("Next: {}",value.next_action.action);println!("{}",escape_terminal_controls(&value.next_action.reason));
    if let (Some(context),Some(subject))=(&value.next_action.review_context_sha256,&value.next_action.review_subject_sha256) {
        println!("Review context: {context}");println!("Final-file subject: {subject}");
    }
}
pub(super) fn dispatch(command:PublishingCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    let value=match command {
        PublishingCommands::Plan {request}=>{let value=publishing::plan(&read(&request)?,&token)?;
            return print_result(name,presentation,&value,||{println!("Fixed publishing intent: {}",value.intent.intent_sha256);println!("Later bindings await actual accepted predecessors and supplied final review");});},
        PublishingCommands::Run {request,run_directory}=>{
            let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            publishing::run(read(&request)?,&root,&token,|_,event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?
        },
        PublishingCommands::Resume {run_directory}=>{
            let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            publishing::resume(&root,&token,|_,event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?
        },
        PublishingCommands::Status {run_directory}=>publishing::status(&absolute_delivery_path(run_directory)?,&token)?,
        PublishingCommands::NextAction {run_directory}=>{let value=publishing::next_action(&absolute_delivery_path(run_directory)?,&token)?;
            return print_result(name,presentation,&value,||{println!("Next: {}",value.action);println!("{}",escape_terminal_controls(&value.reason));});},
        PublishingCommands::RecordReview {run_directory,declaration}=>{
            let review=read_delivery_document(&declaration,"supplied exact-final-file posting decision")?;
            publishing::record_review(&absolute_delivery_path(run_directory)?,&review,&token)?
        },
    };
    print_result(name,presentation,&value,||human(&value))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publishing_requires_explicit_request_root_and_separate_supplied_review() {
        let args=["aniflow","delivery","publishing","run","--request","publishing.json"];
        assert!(Cli::try_parse_from(args).is_err());assert!(Cli::try_parse_from(args.into_iter().chain(["--run-directory","private/publish"])).is_ok());
        let review=["aniflow","delivery","publishing","record-review","--run-directory","private/publish"];
        assert!(Cli::try_parse_from(review).is_err());assert!(Cli::try_parse_from(review.into_iter().chain(["--declaration","my-decision.json"])).is_ok());
    }
    #[test]
    fn status_and_advice_do_not_accept_execution_review_posting_or_signing_flags() {
        for verb in ["status","next-action"] {
            let args=["aniflow","delivery","publishing",verb,"--run-directory","private/publish"];
            assert!(Cli::try_parse_from(args).is_ok());
            for flag in ["--approve","--execute","--post","--sign","--install","--download"] {assert!(Cli::try_parse_from(args.into_iter().chain([flag])).is_err());}
        }
    }
}
