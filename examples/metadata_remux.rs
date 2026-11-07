//! Authored SDK/CLI handoff; not executed or native-qualified.
use std::{fs::File,io::Read,path::PathBuf};
fn bounded(path:&PathBuf)->Result<Vec<u8>,Box<dyn std::error::Error>> {
    let mut bytes=Vec::new();File::open(path)?.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len()>65_536{return Err("caller document exceeds 64 KiB".into());}Ok(bytes)
}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args=std::env::args_os().skip(1).map(PathBuf::from).collect::<Vec<_>>();
    if args.len()!=6{return Err("usage: metadata_remux SOURCE SETTINGS SELECTION ANIFLOW_EXECUTABLE RUN_PARENT_OR_ROOT plan|run|resume".into());}
    let settings=aniflow::video_assembly::AssemblyConfiguration::from_json_slice(&bounded(&args[1])?)?;
    let selection=serde_json::from_slice(&bounded(&args[2])?)?;
    let request=aniflow::delivery_metadata::remux::RemuxRequest::new(args[0].clone(),selection,settings,args[3].clone());
    let token=aniflow::CancellationToken::default();
    if args[5].as_os_str()=="plan" {println!("{}",serde_json::to_string_pretty(&aniflow::delivery_metadata::remux::plan(&request,&token)?)?);return Ok(());}
    let result=if args[5].as_os_str()=="run" {aniflow::delivery_metadata::remux::run(request,args[4].clone(),&token,|_|{})?}
        else if args[5].as_os_str()=="resume" {aniflow::delivery_metadata::remux::resume(request,&args[4],&token,|_|{})?}else{return Err("mode must be plan, run or resume".into());};
    println!("{}",serde_json::to_string_pretty(&result)?);
    println!("{}",serde_json::to_string_pretty(&aniflow::delivery_metadata::remux::public_summary(&result.run_directory)?)?);Ok(())
}
