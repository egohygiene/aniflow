use super::*;
use aniflow::delivery_audio::{policy::Policy,policy_check};

#[derive(Debug,Subcommand)]
pub(super) enum AudioPolicyCommands {
    Describe {#[arg(long)] policy:PathBuf},
    Check {#[arg(long)] request:PathBuf},
}
impl AudioPolicyCommands {
    pub(super) fn name(&self)->CommandName {match self {
        Self::Describe {..}=>CommandName::DeliveryAudioPolicyDescribe,
        Self::Check {..}=>CommandName::DeliveryAudioPolicyCheck,
    }}
}
pub(super) fn dispatch(command:AudioPolicyCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();
    match command {
        AudioPolicyCommands::Describe {policy}=>{
            let policy=Policy::from_json_slice(&read_audio_configuration(&policy,"selected audio delivery policy")?)?;
            let digest=policy.identity_sha256()?;
            let value=serde_json::json!({"schema":"aniflow.audio-policy-description/v1","policy":policy,"policy_sha256":digest,"measurement_performed":false,"processing_performed":false});
            print_result(name,presentation,&value,||println!("Explicit audio policy: {}; no measurement or processing performed",value["policy"]["id"]))
        },
        AudioPolicyCommands::Check {request}=>{
            let request=policy_check::Request::from_json_slice(&read_audio_configuration(&request,"audio policy check request")?)?;
            let result=policy_check::assess(&request,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Integrated loudness: {:?}; true peak: {:?}; matches selected policy: {}",result.evaluation.integrated_loudness.state,result.evaluation.true_peak.state,result.evaluation.matches_selected_policy))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_policy_or_current_qc_request_is_required_without_mutation_flags() {
        assert!(Cli::try_parse_from(["aniflow","delivery","audio-policy","describe"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","delivery","audio-policy","check","--request","qc target.json"]).is_ok());
        assert!(Cli::try_parse_from(["aniflow","delivery","audio-policy","check","--request","qc.json","--normalize"]).is_err());
    }
}
