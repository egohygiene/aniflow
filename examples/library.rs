use std::env;
use std::path::PathBuf;

use aniflow::RunRequest;
use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let mut arguments = env::args_os().skip(1);
    let operation = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .context(
            "usage: cargo run --example library -- <doctor|inspect|plan|run|resume|status> ...",
        )?;

    match operation.as_str() {
        "doctor" => {
            let pipeline = arguments.next().map(PathBuf::from);
            println!("{:#?}", aniflow::doctor(pipeline.as_deref())?);
        }
        "inspect" => {
            let input = next_path(&mut arguments, "input")?;
            println!("{:#?}", aniflow::inspect(input)?);
        }
        "plan" => {
            let input = next_path(&mut arguments, "input")?;
            let pipeline = next_path(&mut arguments, "pipeline")?;
            println!("{:#?}", aniflow::plan(input, pipeline)?);
        }
        "run" => {
            let input = next_path(&mut arguments, "input")?;
            let pipeline = next_path(&mut arguments, "pipeline")?;
            println!("{:#?}", aniflow::run(RunRequest::new(input, pipeline))?);
        }
        "resume" => {
            let run_directory = next_path(&mut arguments, "run directory")?;
            println!("{:#?}", aniflow::resume(run_directory)?);
        }
        "status" => {
            let run_directory = next_path(&mut arguments, "run directory")?;
            println!("{:#?}", aniflow::status(run_directory)?);
        }
        other => bail!("unknown library example operation `{other}`"),
    }

    Ok(())
}

fn next_path(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    name: &str,
) -> Result<PathBuf> {
    arguments
        .next()
        .map(PathBuf::from)
        .with_context(|| format!("missing {name}"))
}
