//! Explicit local #35 composition example. Authored; not executed/qualified.
use std::path::Path;
use aniflow::delivery_audio::source_signal::{measure,VideoAudioSignalRequest};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let arguments=std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.len()!=2 {return Err("usage: video_audio_signal REQUEST.json /absolute/fresh-observation-directory".into());}
    let bytes=std::fs::read(&arguments[0])?;
    if bytes.len()>65_536{return Err("request exceeds 64 KiB".into());}
    let request:VideoAudioSignalRequest=serde_json::from_slice(&bytes)?;
    let result=measure(&request,Path::new(&arguments[1]),&aniflow::CancellationToken::new())?;
    println!("{}",serde_json::to_string_pretty(&result)?);Ok(())
}
