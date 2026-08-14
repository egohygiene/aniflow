use std::path::Path;

use aniflow::{ErrorCategory, Result, RunProgress, RunRequest};

#[allow(dead_code)]
fn independent_consumer(input: &Path, pipeline: &Path, run_directory: &Path) -> Result<()> {
    let _diagnostics = aniflow::doctor(Some(pipeline))?;
    let _inspection = aniflow::inspect(input)?;
    let _plan = aniflow::plan(input, pipeline)?;
    let outcome = aniflow::run(RunRequest::new(input, pipeline))?;
    let _status = aniflow::status(&outcome.run_directory)?;
    let _resumed = aniflow::resume(run_directory)?;
    Ok(())
}

#[test]
fn crate_root_exposes_the_complete_application_facade() {
    let consumer: fn(&Path, &Path, &Path) -> Result<()> = independent_consumer;
    let _ = consumer;
}

#[test]
fn run_request_builder_is_consumer_friendly() {
    let request = RunRequest::new("source.mp4", "pipeline.yml").with_output_directory("runs");

    assert_eq!(request.input, Path::new("source.mp4"));
    assert_eq!(request.pipeline, Path::new("pipeline.yml"));
    assert_eq!(request.output_directory.as_deref(), Some(Path::new("runs")));
}

#[test]
fn observed_execution_is_available_without_cli_types() {
    fn observe(_: &RunProgress) {}

    let request = RunRequest::new("missing-source.mp4", "missing-pipeline.yml");
    let error = aniflow::run_with_progress(request, observe)
        .expect_err("a missing source must prevent the run");

    assert_eq!(error.category(), ErrorCategory::Input);
    assert!(error.to_string().contains("input video does not exist"));
}

#[test]
fn facade_maps_invalid_requests_to_stable_categories() {
    let doctor_error = aniflow::doctor(Some(Path::new("missing-pipeline.yml")))
        .expect_err("a missing pipeline must prevent diagnostics");
    let status_error =
        aniflow::status("missing-run").expect_err("a missing run directory must prevent status");

    assert_eq!(doctor_error.category(), ErrorCategory::Configuration);
    assert_eq!(status_error.category(), ErrorCategory::State);
}
