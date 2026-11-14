use super::*;
use crate::destination::local;
use crate::frame_io::{copy_pinned, inventory, require_absent};
use crate::{
    ArtifactKind, ArtifactRole, ProviderInvocationArtifactBinding, ProviderInvocationRequest,
    StreamRole,
};
fn binding<'a>(
    values: &'a [ProviderInvocationArtifactBinding],
    port: &str,
) -> Result<&'a ProviderInvocationArtifactBinding> {
    let selected = values.iter().filter(|v| v.port == port).collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err(invalid("historical import port missing or ambiguous"));
    }
    Ok(selected[0])
}
fn validate_binding(
    value: &ProviderInvocationArtifactBinding,
    kind: ArtifactKind,
    role: ArtifactRole,
    stream: StreamRole,
    media: &str,
) -> Result<()> {
    if value.kind != kind
        || value.artifact_role != role
        || value.stream_role != Some(stream)
        || value.artifact_type != media
    {
        return Err(invalid(
            "historical import artifact differs from the fixed port profile",
        ));
    }
    Ok(())
}
fn subset(op: &ImportOperation) -> Result<ImportedSpatialSelection> {
    Ok(ImportedSpatialSelection {schema:SUBSET_SCHEMA.into(),source_sha256:op.intent.source_sha256.clone(),source_index_sha256:op.intent.source_index_sha256.clone(),original_frame_count:op.intent.original_frame_count,
        output_width:op.intent.settings.output_width,output_height:op.intent.settings.output_height,frames:op.intent.frames.clone(),import_decision_sha256:canonical_sha256(&op.declaration)?,
        historical_processing:"unknown; native importer observed these bytes now, not their prior processor/model/settings".into(),source_relationship:"operator_declared; original clock retained; creative quality not approved".into()})
}
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    request.validate()?;
    if !cfg!(unix) {
        return Err(invalid(
            "historical import requires the existing Unix process-group runtime",
        ));
    }
    let operation: ImportOperation = serde_json::from_value(
        serde_json::to_value(&request.configuration.values)
            .map_err(|_| invalid("invalid historical configuration"))?,
    )
    .map_err(|_| invalid("invalid closed historical operation"))?;
    operation.validate()?;
    if operation.configuration()? != request.configuration
        || request.inputs.len() != 2 + operation.intent.frames.len()
        || request.outputs.len() != 3
    {
        return Err(invalid(
            "historical operation/port set differs from the exact lock",
        ));
    }
    let source = binding(&request.inputs, "source")?;
    let index = binding(&request.inputs, "index")?;
    validate_binding(
        source,
        ArtifactKind::File,
        ArtifactRole::TemporalSource,
        StreamRole::Video,
        "video/*",
    )?;
    validate_binding(
        index,
        ArtifactKind::File,
        ArtifactRole::ValidationEvidence,
        StreamRole::TimedMetadata,
        "application/vnd.aniflow.rife-frame-index+json",
    )?;
    let mut files = Vec::new();
    for frame in &operation.intent.frames {
        let id = format!("historical_frame_{:06}", frame.ordinal);
        let selected = request
            .inputs
            .iter()
            .filter(|v| v.port == "frames" && v.artifact_id == id)
            .collect::<Vec<_>>();
        if selected.len() != 1 {
            return Err(invalid(
                "selected historical frame binding missing or duplicated",
            ));
        }
        validate_binding(
            selected[0],
            ArtifactKind::File,
            ArtifactRole::Intermediate,
            StreamRole::Video,
            "image/png",
        )?;
        files.push(SelectedFile {
            ordinal: frame.ordinal,
            file: selected[0].path.clone(),
            sha256: frame.sha256.clone(),
            byte_count: frame.byte_count,
        });
    }
    let token = CancellationToken::default();
    let rebound = ImportRequest {
        schema: "aniflow.historical-spatial-import-request/v1".into(),
        source_video: source.path.clone(),
        source_index: index.path.clone(),
        frames: files,
        settings: operation.intent.settings.clone(),
        provider_executable: PathBuf::new(),
    };
    if review_plan(&rebound, &token)?.intent != operation.intent {
        return Err(invalid(
            "historical source/index/selection changed before validation",
        ));
    }
    let frames = binding(&request.outputs, "frames")?;
    let output_index = binding(&request.outputs, "index")?;
    let evidence = binding(&request.outputs, "evidence")?;
    validate_binding(
        frames,
        ArtifactKind::Directory,
        ArtifactRole::Intermediate,
        StreamRole::Video,
        "application/vnd.aniflow.historical-spatial-frames",
    )?;
    validate_binding(
        output_index,
        ArtifactKind::File,
        ArtifactRole::ValidationEvidence,
        StreamRole::TimedMetadata,
        "application/vnd.aniflow.imported-spatial-selection+json",
    )?;
    validate_binding(
        evidence,
        ArtifactKind::File,
        ArtifactRole::ValidationEvidence,
        StreamRole::TimedMetadata,
        "application/vnd.aniflow.historical-spatial-report+json",
    )?;
    let parent = frames
        .path
        .parent()
        .ok_or_else(|| invalid("historical outputs need an existing parent"))?;
    if output_index.path.parent() != Some(parent)
        || evidence.path.parent() != Some(parent)
        || frames.path == output_index.path
        || frames.path == evidence.path
        || evidence.path == output_index.path
    {
        return Err(invalid(
            "historical outputs must be distinct fixed siblings",
        ));
    }
    for out in [&frames.path, &output_index.path, &evidence.path] {
        require_absent(out)?;
        for input in request
            .inputs
            .iter()
            .map(|v| &v.path)
            .chain(std::iter::once(
                &operation.intent.settings.decoder.ffmpeg.executable,
            ))
        {
            if out.starts_with(input) || input.starts_with(out) {
                return Err(invalid("historical candidates overlap an input or tool"));
            }
        }
    }
    crate::frame_io::ensure_parent(parent)?;
    let tool = &operation.intent.settings.decoder.ffmpeg;
    crate::audio_inspection::process::verify_pin(tool, "ffmpeg")
        .map_err(|_| invalid("historical decoder pin changed"))?;
    let tool_pin = identity(&tool.executable, 134_217_728)?;
    let scratch = tempfile::Builder::new()
        .prefix(".historical-import-")
        .tempdir_in(parent)
        .map_err(|_| invalid("cannot reserve historical decoder scratch"))?;
    let scratch_root = scratch
        .path()
        .canonicalize()
        .map_err(|_| invalid("cannot resolve historical decoder scratch"))?;
    let mut decoder = tool.clone();
    decoder.executable = scratch_root.join("ffmpeg");
    copy_pinned(
        &tool.executable,
        &decoder.executable,
        &tool_pin,
        134_217_728,
        &token,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&decoder.executable, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| invalid("cannot prepare private historical decoder"))?;
    }
    let limits = operation.intent.settings.decoder.tool_limits();
    crate::frame_decode::verify_version(&decoder, &limits, &token, &scratch_root)?;
    local::directory(&frames.path)?;
    let mut decoded = Vec::new();
    for (input, frame) in rebound.frames.iter().zip(&operation.intent.frames) {
        let destination = frames.path.join(&frame.filename);
        copy_pinned(
            &input.file,
            &destination,
            &(frame.sha256.clone(), frame.byte_count),
            operation.intent.settings.decoder.maximum_frame_bytes,
            &token,
        )?;
        decoded.push(crate::frame_decode::decode(
            &decoder,
            &limits,
            &token,
            &scratch_root,
            &destination,
            operation.intent.settings.output_width,
            operation.intent.settings.output_height,
        )?);
        if inspect_png(
            &destination,
            operation.intent.settings.output_width,
            operation.intent.settings.output_height,
            operation.intent.settings.decoder.maximum_frame_bytes,
        )? != (frame.sha256.clone(), frame.byte_count)
        {
            return Err(invalid("imported historical PNG changed during decode"));
        }
    }
    let names = operation
        .intent
        .frames
        .iter()
        .map(|f| f.filename.clone().into())
        .collect::<Vec<_>>();
    inventory(&frames.path, &names)?;
    if review_plan(&rebound, &token)?.intent != operation.intent
        || identity(&tool.executable, 134_217_728)? != tool_pin
        || identity(&decoder.executable, 134_217_728)? != tool_pin
    {
        return Err(invalid(
            "historical inputs or decoder changed before evidence publication",
        ));
    }
    let subset = subset(&operation)?;
    local::write(&output_index.path, &local::bytes(&subset)?)?;
    let report = ImportReport {
        schema: "aniflow.historical-spatial-import-report/v1".into(),
        operation,
        provider_lock_sha256: request.provider_lock_sha256.clone(),
        decoded_rgb24_sha256: decoded,
        subset,
        native_qualified: false,
    };
    local::write(&evidence.path, &local::bytes(&report)?)?;
    Ok(())
}
/// Current ordinary native acceptance plus exact report, subset and frame bytes.
pub fn observe(root: &Path, token: &CancellationToken) -> Result<ImportReport> {
    check_cancelled(token)?;
    ensure_canonical_directory(root)?;
    let before = crate::status_v3(root)?;
    if before.payload.state != crate::PipelineV3RunState::Complete
        || before.payload.delivery.is_none()
    {
        return Err(invalid("historical import is not currently complete"));
    }
    let workspace = crate::PipelineV3Workspace::open_read_only(root)?;
    let value: serde_json::Value = local::read_bounded(&workspace.plan(), 67_108_864)?;
    let plan = PipelineV3Plan::from_json_slice(&canonical_json_bytes(&value)?)
        .map_err(|e| invalid(e.message))?;
    if plan.payload.pipeline_name != "explicit-historical-spatial-import"
        || plan.payload.stages.len() != 1
        || plan.payload.stages[0].provider_lock.payload.provider.id != PROVIDER_ID
        || plan.plan_sha256 != before.payload.plan_sha256
    {
        return Err(invalid("not the exact historical native import plan"));
    }
    let locate = |id: &str| -> Result<PathBuf> {
        let selected = before
            .payload
            .outputs
            .iter()
            .filter(|a| a.id == id)
            .collect::<Vec<_>>();
        if selected.len() != 1 {
            return Err(invalid("historical native output missing or ambiguous"));
        }
        let a = selected[0];
        let path = root.join(
            a.relative_path
                .as_deref()
                .ok_or_else(|| invalid("historical output locator missing"))?,
        );
        if !path.starts_with(root) {
            return Err(invalid("historical output outside its native root"));
        }
        Ok(path)
    };
    let report_path = locate("historical_evidence")?;
    let report: ImportReport = local::read(&report_path)?;
    report.operation.validate()?;
    if report.schema != "aniflow.historical-spatial-import-report/v1"
        || report.native_qualified
        || report.provider_lock_sha256 != plan.payload.stages[0].provider_lock.lock_sha256
        || report
            .operation
            .configuration()?
            .effective_configuration_sha256
            != plan.payload.stages[0]
                .provider_lock
                .payload
                .effective_configuration_sha256
        || report.subset != subset(&report.operation)?
        || report.decoded_rgb24_sha256.len() != report.subset.frames.len()
    {
        return Err(invalid(
            "historical report differs from its exact accepted operation",
        ));
    }
    for hash in &report.decoded_rgb24_sha256 {
        require_sha256(hash, "accepted imported pixels")?;
    }
    let saved_subset: ImportedSpatialSelection = local::read(&locate("historical_index")?)?;
    if saved_subset != report.subset {
        return Err(invalid("historical subset differs from its report"));
    }
    let frames = locate("historical_frames")?;
    let names = report
        .subset
        .frames
        .iter()
        .map(|f| f.filename.clone().into())
        .collect::<Vec<_>>();
    inventory(&frames, &names)?;
    for f in &report.subset.frames {
        check_cancelled(token)?;
        if identity(
            &frames.join(&f.filename),
            report.operation.intent.settings.decoder.maximum_frame_bytes,
        )? != (f.sha256.clone(), f.byte_count)
        {
            return Err(invalid("accepted historical PNG changed"));
        }
    }
    if crate::status_v3(root)? != before {
        return Err(invalid(
            "historical native state changed during observation",
        ));
    }
    Ok(report)
}
