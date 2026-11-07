//! Explicit operator-supplied declaration; no automatic approval or native work.
use std::path::Path;
use aniflow::music_video_recipe::manual::{self,ManualContext,ManualReview};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let arguments=std::env::args().skip(1).collect::<Vec<_>>();
    if !matches!(arguments.len(),2|3){return Err("usage: manual_review CHECKPOINT EXPECTED_CONTEXT.json [OPERATOR_DECISION.json]".into());}
    let read=|path:&str|->Result<Vec<u8>,Box<dyn std::error::Error>> {let bytes=std::fs::read(path)?;if bytes.len()>65_536{return Err("manual document exceeds 64 KiB".into());}Ok(bytes)};
    let context:ManualContext=serde_json::from_slice(&read(&arguments[1])?)?;
    let result=if let Some(path)=arguments.get(2) {let decision:ManualReview=serde_json::from_slice(&read(path)?)?;
        manual::review(Path::new(&arguments[0]),&context,&decision)?
    } else {manual::status(Path::new(&arguments[0]),&context)?};
    println!("{}",serde_json::to_string_pretty(&result)?);Ok(())
}
