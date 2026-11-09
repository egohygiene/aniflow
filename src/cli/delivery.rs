//! Thin delivery over accepted-media destination preparation.
use super::*;
use aniflow::destination;
#[derive(Debug,Subcommand)]
pub(super) enum DeliveryCommands {
    /// Read current accepted facts against explicit dated supplied requirements.
    Preflight {#[arg(long)] request:PathBuf},
}
impl DeliveryCommands {
    pub(super) fn name(&self)->CommandName {match self {Self::Preflight {..}=>CommandName::DeliveryPreflight}}
}
pub(super) fn dispatch(command:DeliveryCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();match command {
        DeliveryCommands::Preflight {request}=>{
            let request=destination::PreflightRequest::from_json_slice(&read_audio_configuration(&request,"destination preflight request")?)?;
            let result=destination::preflight(&request,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,|| {
                println!("Destination preflight: {:?}",result.payload.disposition);
                for check in &result.payload.checks {println!("  {}: {:?}; {}",check.id,check.state,check.reason);}
                println!("Supplied requirement evidence; human review and publication remain separate");
            })
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preflight_is_explicit_bounded_input_without_transform_or_post_flags() {
        let base=["aniflow","--output","json","delivery","preflight","--request","request 音.json"];
        assert_eq!(Cli::try_parse_from(base).unwrap().command.name(),CommandName::DeliveryPreflight);
        for flag in ["--post","--normalize","--trim","--resize","--download","--approve"] {assert!(Cli::try_parse_from(base.into_iter().chain([flag])).is_err());}
        assert!(Cli::try_parse_from(["aniflow","delivery","preflight"]).is_err());
    }
}
