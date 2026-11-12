use super::*;
use aniflow::delivery_audio::gain;
#[derive(Debug,Subcommand)]
pub(super) enum AudioGainCommands {
    Plan {#[arg(long)] request:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    Resume {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Status {#[arg(long)] run_directory:PathBuf},
}
impl AudioGainCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Plan {..}=>CommandName::DeliveryAudioGainPlan,Self::Run {..}=>CommandName::DeliveryAudioGainRun,
    Self::Resume {..}=>CommandName::DeliveryAudioGainResume,Self::Status {..}=>CommandName::DeliveryAudioGainStatus,
}}}
pub(super) fn dispatch(command:AudioGainCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    let request=|path:&Path|gain::Request::from_json_slice(&read_audio_configuration(path,"explicit audio gain request")?);
    match command {
        AudioGainCommands::Plan {request:path}=>{let value=gain::plan(&request(&path)?,&token)?;print_result(name,presentation,&value,||println!("Fixed gain decision: {:?}; predicted peak is not measured output",value.decision.state))},
        AudioGainCommands::Run {request:path,output_directory}=>{
            let request=request(&path)?;let parent=absolute_delivery_path(output_directory)?;let mut recovery=None;
            let value=gain::run(&request,&parent,&token,|event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,recovery))?;
            print_result(name,presentation,&value,||print_pipeline_v3_outcome(&value))
        },
        AudioGainCommands::Resume {request:path,run_directory}=>{
            let request=request(&path)?;let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            let value=gain::resume(&request,&root,&token,|event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,recovery))?;
            print_result(name,presentation,&value,||print_pipeline_v3_outcome(&value))
        },
        AudioGainCommands::Status {run_directory}=>{let value=gain::observe(&absolute_delivery_path(run_directory)?,&token)?;print_result(name,presentation,&value,||println!("Gain candidate retained; fresh exact-file QC and musical review required"))},
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn gain_requires_separate_output_and_never_accepts_implicit_limiter_or_source_replacement() {
        let args=["aniflow","delivery","audio-gain","run","--request","gain.json"];
        assert!(Cli::try_parse_from(args).is_err());assert!(Cli::try_parse_from(args.into_iter().chain(["--output-directory","new candidates"])).is_ok());
        for flag in ["--limiter","--replace-source","--resample","--upload"] {assert!(Cli::try_parse_from(args.into_iter().chain(["--output-directory","new candidates",flag])).is_err());}
    }
}
