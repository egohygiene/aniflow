use std::fs;
use std::path::{Path, PathBuf};

use aniflow::{
    CancellationToken, ErrorCategory, PipelineInputBinding, PipelinePlanningContext,
    PipelinePlanningFailure, PipelineV3Configuration, PipelineV3Plan, ProviderRegistry, Result,
    RunProgress, RunRequest,
};
use tempfile::TempDir;

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

#[allow(dead_code)]
fn independent_pipeline_v3_consumer(
    pipeline: PathBuf,
    input_bindings: &[PipelineInputBinding],
    provider_registration_paths: &[PathBuf],
    context: &PipelinePlanningContext,
) -> std::result::Result<PipelineV3Plan, PipelinePlanningFailure> {
    aniflow::plan_v3(
        pipeline,
        input_bindings,
        provider_registration_paths,
        context,
    )
}

#[test]
fn crate_root_exposes_the_complete_application_facade() {
    let consumer: fn(&Path, &Path, &Path) -> Result<()> = independent_consumer;
    let _ = consumer;
}

#[test]
fn crate_root_exposes_pipeline_v3_planning_contracts() {
    let resolver: fn(
        &PipelineV3Configuration,
        &[PipelineInputBinding],
        &ProviderRegistry,
        &PipelinePlanningContext,
    ) -> std::result::Result<PipelineV3Plan, PipelinePlanningFailure> =
        aniflow::resolve_pipeline_v3;
    let _ = resolver;
    let file_planner = independent_pipeline_v3_consumer;
    let _ = file_planner;
    assert_eq!(aniflow::PIPELINE_V3_SCHEMA, "aniflow.pipeline/v3");
    assert_eq!(
        aniflow::PIPELINE_V3_PLAN_SCHEMA_V1,
        "aniflow.pipeline-plan/v1"
    );
    assert_eq!(
        aniflow::PROVIDER_REGISTRATION_SCHEMA_V1,
        "aniflow.provider-registration/v1"
    );
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
fn pipeline_execution_accepts_a_shared_cancellation_token() {
    fn observe(_: &RunProgress) {}

    let request = RunRequest::new("missing-source.mp4", "missing-pipeline.yml");
    let cancellation = CancellationToken::default();
    let error = aniflow::run_with_progress_and_cancellation(request, &cancellation, observe)
        .expect_err("a missing source must prevent the run");

    assert_eq!(error.category(), ErrorCategory::Input);
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

#[test]
fn pipeline_v3_run_and_resume_fail_before_workspace_mutation() {
    let temporary = TempDir::new().expect("temporary directory should be created");
    let source = temporary.path().join("source.bin");
    let pipeline = temporary.path().join("pipeline-v3.yml");
    let runs = temporary.path().join("runs");
    fs::write(&source, b"immutable source").expect("source fixture should be written");
    fs::write(
        &pipeline,
        "schema: aniflow.pipeline/v3\nname: execution-is-not-enabled\n",
    )
    .expect("Pipeline v3 fixture should be written");

    let run_error = aniflow::run(RunRequest::new(&source, &pipeline).with_output_directory(&runs))
        .expect_err("Pipeline v3 execution must be rejected");
    assert_eq!(run_error.category(), ErrorCategory::Configuration);
    assert!(run_error.message().contains("plan-v3"));
    assert!(!runs.exists());
    assert_eq!(
        fs::read(&source).expect("source should remain readable"),
        b"immutable source"
    );

    let run_directory = temporary.path().join("v3-run");
    let config_directory = run_directory.join("config");
    fs::create_dir_all(&config_directory).expect("resume fixture should be created");
    fs::write(
        config_directory.join("pipeline.yml"),
        fs::read(&pipeline).expect("Pipeline v3 fixture should remain readable"),
    )
    .expect("snapshotted Pipeline v3 fixture should be written");
    let resume_error = aniflow::resume(&run_directory)
        .expect_err("Pipeline v3 resume must be rejected before opening a workspace");
    assert_eq!(resume_error.category(), ErrorCategory::Configuration);
    assert!(resume_error.message().contains("plan-v3"));
    assert!(!run_directory.join("logs").exists());
}
