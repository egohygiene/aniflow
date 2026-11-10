use super::*;
use aniflow::video_interpolation::chunks::{self,Request};
#[derive(Debug,Subcommand)]
pub(super) enum ChunkCommands {
    Plan {#[arg(long)] request:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Resume {#[arg(long)] run_directory:PathBuf},Status {#[arg(long)] run_directory:PathBuf},
    BindAssembly {#[arg(long)] run_directory:PathBuf,#[arg(long)] output:PathBuf},
}
impl ChunkCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Plan {..}=>CommandName::VideoRifeChunksPlan,Self::Run {..}=>CommandName::VideoRifeChunksRun,
    Self::Resume {..}=>CommandName::VideoRifeChunksResume,Self::Status {..}=>CommandName::VideoRifeChunksStatus,
    Self::BindAssembly {..}=>CommandName::VideoRifeChunksBindAssembly,
}}}
fn read(path:&Path)->Result<Request>{Request::from_json_slice(&read_bounded_configuration(path,"chunked recovered RIFE request",65_536)?)}
pub(super) fn dispatch(command:ChunkCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    let result=match command {
        ChunkCommands::Plan {request}=>{let value=chunks::plan(&read(&request)?,&token)?;return print_result(name,presentation,&value,||println!("Selected native chunk graph: {}",value.intent_sha256));},
        ChunkCommands::BindAssembly {run_directory,output}=>{let value=chunks::bind_assembly(&absolute_delivery_path(run_directory)?,&absolute_delivery_path(output)?,&token)?;return print_result(name,presentation,&value,||println!("Complete accepted recovered RIFE assembly carrier; review remains separate"));},
        ChunkCommands::Run {request,run_directory}=>{let root=absolute_delivery_path(run_directory)?;let request=read(&request)?;let mut recovery=Some(root.clone());chunks::run(&request,&root,&token,|e|observe_pipeline_v3_progress(presentation,&mut recovery,e)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?},
        ChunkCommands::Resume {run_directory}=>{let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());chunks::resume(&root,&token,|e|observe_pipeline_v3_progress(presentation,&mut recovery,e)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?},
        ChunkCommands::Status {run_directory}=>chunks::status(&absolute_delivery_path(run_directory)?,&token)?,
    };print_result(name,presentation,&result,||println!("Accepted chunks: {}; pending: {}; complete: {}",result.completed_chunks,result.pending_chunks,result.complete))
}
#[cfg(test)] mod tests {use super::*;
#[test] fn chunk_resume_has_no_replacement_request_or_file_adoption() {
    assert!(Cli::try_parse_from(["aniflow","video","rife","recovered","chunks","run","--request","chunks.json"]).is_err());
    assert!(Cli::try_parse_from(["aniflow","video","rife","recovered","chunks","resume","--run-directory","private/chunks"]).is_ok());
    assert!(Cli::try_parse_from(["aniflow","video","rife","recovered","chunks","resume","--run-directory","private/chunks","--request","changed.json"]).is_err());
}}
