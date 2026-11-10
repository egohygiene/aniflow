use super::*;
use aniflow::amv_advisory::{self as advisory,Request,Packet,Candidate};
#[derive(Debug,Subcommand)]
pub(super) enum AdvisoryCommands {Prepare {#[arg(long)] request:PathBuf},Notes {#[arg(long)] request:PathBuf,#[arg(long)] packet:PathBuf,#[arg(long)] candidate:PathBuf}}
impl AdvisoryCommands {pub(super) fn name(&self)->CommandName {match self {Self::Prepare {..}=>CommandName::VideoAdvisoryPrepare,Self::Notes {..}=>CommandName::VideoAdvisoryNotes}}}
fn read(path:&Path)->Result<Request>{Request::from_json_slice(&read_bounded_configuration(path,"AMV advisory request",16_384)?)}
pub(super) fn dispatch(command:AdvisoryCommands,presentation:Presentation)->CommandResult<()> {let name=command.name();let token=cli_cancellation_token()?;match command {
    AdvisoryCommands::Prepare {request}=>{let value=advisory::prepare(&read(&request)?,&token)?;print_result(name,presentation,&value,||println!("Evidence packet enabled: {}; no media samples or model invocation",value.enabled))},
    AdvisoryCommands::Notes {request,packet,candidate}=>{let packet=Packet::from_json_slice(&read_bounded_configuration(&packet,"AMV evidence packet",65_536)?)?;let candidate=Candidate::from_json_slice(&read_bounded_configuration(&candidate,"supplied AMV notes",65_536)?)?;let value=advisory::validate_candidate(&read(&request)?,&packet,&candidate,&token)?;print_result(name,presentation,&value,||println!("Cited advisory {}; technical acceptance and human approval remain separate",value.advisory_sha256))},
}}
#[cfg(test)] mod tests {use super::*;
#[test] fn advisory_requires_current_source_packet_and_candidate(){assert!(Cli::try_parse_from(["aniflow","video","advisory","notes","--request","r.json","--packet","p.json"]).is_err());assert!(Cli::try_parse_from(["aniflow","video","advisory","notes","--request","r.json","--packet","p.json","--candidate","c.json"]).is_ok());assert!(Cli::try_parse_from(["aniflow","video","advisory","prepare","--request","r.json","--auto-approve"]).is_err());}}
