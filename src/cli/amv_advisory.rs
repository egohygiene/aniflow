use super::*;
use aniflow::amv_advisory::{self as advisory,Request,Packet,Candidate,runtime};

#[derive(Debug,Subcommand)]
pub(super) enum AdvisoryCommands {
    Prepare {#[arg(long)] request:PathBuf},
    Notes {#[arg(long)] request:PathBuf,#[arg(long)] packet:PathBuf,#[arg(long)] candidate:PathBuf},
    /// Explicit one-shot local model notes; host observations are supplied separately.
    RunLocal {#[arg(long)] request:PathBuf,#[arg(long)] configuration:PathBuf,#[arg(long)] host:PathBuf,#[arg(long)] output_directory:PathBuf},
}
impl AdvisoryCommands {
    pub(super) fn name(&self)->CommandName {match self {
        Self::Prepare {..}=>CommandName::VideoAdvisoryPrepare,
        Self::Notes {..}=>CommandName::VideoAdvisoryNotes,
        Self::RunLocal {..}=>CommandName::VideoAdvisoryRunLocal,
    }}
}
fn read(path:&Path)->Result<Request>{Request::from_json_slice(&read_bounded_configuration(path,"AMV advisory request",16_384)?)}
pub(super) fn dispatch(command:AdvisoryCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
        AdvisoryCommands::Prepare {request}=>{
            let value=advisory::prepare(&read(&request)?,&token)?;
            print_result(name,presentation,&value,||println!("Evidence packet enabled: {}; no media samples or model invocation",value.enabled))
        },
        AdvisoryCommands::Notes {request,packet,candidate}=>{
            let packet=Packet::from_json_slice(&read_bounded_configuration(&packet,"AMV evidence packet",65_536)?)?;
            let candidate=Candidate::from_json_slice(&read_bounded_configuration(&candidate,"supplied AMV notes",65_536)?)?;
            let value=advisory::validate_candidate(&read(&request)?,&packet,&candidate,&token)?;
            print_result(name,presentation,&value,||println!("Cited advisory {}; technical acceptance and human approval remain separate",value.advisory_sha256))
        },
        AdvisoryCommands::RunLocal {request,configuration,host,output_directory}=>{
            let request=read(&request)?;
            // Disabled requests do not read private model or host documents.
            if request.mode==advisory::Mode::Disabled {
                let value=advisory::prepare(&request,&token)?;
                return print_result(name,presentation,&value,||println!("AMV advisory disabled; no provider work"));
            }
            let configuration=runtime::Configuration::from_json_slice(&read_bounded_configuration(&configuration,"AMV local runtime configuration",16_384)?)?;
            let host=if configuration.enabled {serde_json::from_slice::<aniflow::HostResources>(&read_bounded_configuration(&host,"AMV host observations",4096)?).map_err(|_|aniflow::Error::new(aniflow::ErrorCategory::Configuration,"closed explicit AMV host observations required"))?}else{aniflow::HostResources {cpu_threads:1,memory_mib:0,storage_mib:0,gpu_available:false,network_available:false}};
            let value=runtime::run(&request,&configuration,&output_directory,host,&token)?;
            print_result(name,presentation,&value,||println!("Local AMV notes: {:?}; technical and human review remain separate",value.telemetry.state))
        },
    }
}
#[cfg(test)] mod tests {use super::*;
#[test] fn advisory_requires_current_source_packet_and_candidate(){assert!(Cli::try_parse_from(["aniflow","video","advisory","notes","--request","r.json","--packet","p.json"]).is_err());assert!(Cli::try_parse_from(["aniflow","video","advisory","notes","--request","r.json","--packet","p.json","--candidate","c.json"]).is_ok());assert!(Cli::try_parse_from(["aniflow","video","advisory","prepare","--request","r.json","--auto-approve"]).is_err());}
#[test] fn local_notes_requires_explicit_configuration_host_and_private_root(){assert!(Cli::try_parse_from(["aniflow","video","advisory","run-local","--request","r","--configuration","c","--host","h","--output-directory","/private/new"]).is_ok());assert!(Cli::try_parse_from(["aniflow","video","advisory","run-local","--request","r","--configuration","c","--output-directory","/private/new"]).is_err());assert!(Cli::try_parse_from(["aniflow","video","advisory","run-local","--request","r","--configuration","c","--host","h","--output-directory","/private/new","--fallback","cloud"]).is_err());}
}
