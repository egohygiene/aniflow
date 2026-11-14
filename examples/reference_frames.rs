//! Authored manual reference workflow, uncompiled/unrun.
use std::{fs::File, io::Read, path::PathBuf};
fn bounded(path: &PathBuf) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err("caller document exceeds 64 KiB".into());
    }
    Ok(bytes)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if args.len() != 6 {
        return Err("usage: reference_frames SOURCE EXTRACTION_SETTINGS SAMPLING_POLICY ANIFLOW_EXECUTABLE RUN_PARENT_OR_ROOT plan|run|resume".into());
    }
    let settings = aniflow::frame_extraction::FrameExtractionConfiguration::from_json_slice(
        &bounded(&args[1])?,
    )?;
    let policy = serde_json::from_slice(&bounded(&args[2])?)?;
    let request = aniflow::frame_extraction::references::ReferenceRequest::new(
        aniflow::frame_extraction::FrameExtractionRequest::new(&args[0], settings, &args[3]),
        policy,
    );
    let token = aniflow::CancellationToken::default();
    if args[5].as_os_str() == "plan" {
        println!(
            "{}",
            serde_json::to_string_pretty(&aniflow::frame_extraction::references::plan(
                &request, &token
            )?)?
        );
        return Ok(());
    }
    let result = if args[5].as_os_str() == "run" {
        aniflow::frame_extraction::references::run(request, args[4].clone(), &token, |_| {})?
    } else if args[5].as_os_str() == "resume" {
        aniflow::frame_extraction::references::resume(request, &args[4], &token, |_| {})?
    } else {
        return Err("mode must be plan, run or resume".into());
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
