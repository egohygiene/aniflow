use super::*;
use aniflow::destination::video::{self,VideoExportRequest};
#[derive(Debug,Subcommand)]
pub(super) enum VideoExportCommands {Intent {#[arg(long)] request:PathBuf}}
impl VideoExportCommands {pub(super) fn name(&self)->CommandName {match self {Self::Intent {..}=>CommandName::DeliveryVideoExportIntent}}}
pub(super) fn read(path:&Path)->Result<VideoExportRequest> {VideoExportRequest::from_json_slice(&read_audio_configuration(path,"destination video request")?)}
pub(super) fn dispatch(command:VideoExportCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {VideoExportCommands::Intent {request}=>{let value=video::intent(&read(&request)?,&token)?;
        print_result(name,presentation,&value,||{let g=&value.payload.geometry;println!("Destination intent: {}",value.intent_sha256);println!("Scaled content: {} x {}; canvas: {} x {}",g.scaled_width,g.scaled_height,g.width,g.height);println!("Source/clock/audio preserved; lossy quality and platform acceptance require separate checks");})}}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intent_requires_an_explicit_request_without_automatic_transform_flags() {
        let args=["aniflow","delivery","video-export","intent","--request","private intent.json"];
        assert_eq!(Cli::try_parse_from(args).unwrap().command.name(),CommandName::DeliveryVideoExportIntent);
        for flag in ["--crop","--trim","--normalize","--post","--download"] {assert!(Cli::try_parse_from(args.into_iter().chain([flag])).is_err());}
    }
}
