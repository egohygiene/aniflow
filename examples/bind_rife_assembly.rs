//! Source example only; no model is executed by the binding operation.
use std::{fs::File,io::Read,path::PathBuf};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args=std::env::args_os().skip(1).map(PathBuf::from).collect::<Vec<_>>();
    if args.len()!=4{return Err("usage: bind_rife_assembly ACCEPTED_RUN RIFE_CONFIGURATION ORIGINAL_INDEX NEW_CARRIER".into());}
    let mut bytes=Vec::new();File::open(&args[1])?.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len()>65_536{return Err("configuration exceeds 64 KiB".into());}
    let configuration=serde_json::from_slice(&bytes)?;
    let carrier=aniflow::video_assembly::rife::bind_accepted_run(&args[0],configuration,&args[2],&args[3])?;
    println!("frames_directory={}",carrier.validate_binding()?.display());
    println!("frame_index={}",args[3].display());Ok(())
}
