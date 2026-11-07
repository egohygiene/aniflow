//! Explicit local manual lyrics workflow. Authored; uncompiled/unrun.
use std::io::Read;
use std::path::Path;
use aniflow::music_video_recipe::lyrics::{self,ManualLyricsRequest};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let arguments=std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.len()<3||arguments.len()>4{return Err("usage: manual_lyrics init|import|status|export REQUEST.json WORKFLOW [INPUT_OR_OUTPUT]".into());}
    let mut bytes=Vec::new();std::fs::File::open(&arguments[1])?.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len()>65_536{return Err("request exceeds 64 KiB".into());}
    let request:ManualLyricsRequest=serde_json::from_slice(&bytes)?;let root=Path::new(&arguments[2]);
    let selected=arguments.get(3).map(Path::new);let token=aniflow::CancellationToken::default();
    let value=match arguments[0].as_str() {
        "init"=>serde_json::to_value(lyrics::initialize(&request,root,selected,&token)?)?,
        "import"=>serde_json::to_value(lyrics::import(&request,root,selected.ok_or("import needs the manually authored input")?,&token)?)?,
        "status"=>serde_json::to_value(lyrics::status(&request,root)?)?,
        "export"=>serde_json::to_value(lyrics::export(&request,root,selected.ok_or("export needs a fresh output directory")?)?)?,
        _=>return Err("choose init, import, status or export".into()),
    };
    println!("{}",serde_json::to_string_pretty(&value)?);Ok(())
}
