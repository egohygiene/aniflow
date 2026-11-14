use super::*;
use crate::upscayl_recovery::{
    UPSCAYL_CHUNK_CAPABILITY_ID, UPSCAYL_CHUNK_OPERATION_SCHEMA_V1, UPSCAYL_CHUNK_REPORT_SCHEMA_V1,
    UPSCAYL_FRAME_CAPABILITY_ID, UPSCAYL_FRAME_OPERATION_SCHEMA_V1,
    UPSCAYL_MAXIMUM_CHUNK_REPORT_BYTES, UPSCAYL_MAXIMUM_FRAME_REPORT_BYTES,
    UPSCAYL_SELECTED_FRAME_OPERATION_SCHEMA_V2, UPSCAYL_SELECTED_FRAME_REPORT_SCHEMA_V2,
    UpscaylChunkOperation, UpscaylChunkReport, UpscaylFrameOperation, UpscaylFrameReport,
    UpscaylFrameSpec, historical,
};
use crate::{
    ArtifactEvidence, ArtifactKind, PipelineV3Plan, PipelineV3RunManifest, PipelineV3StageState,
    ResolvedPipelineStage, StageCheckpoint,
};
fn read_plan(root: &Path) -> Result<PipelineV3Plan> {
    let workspace = crate::PipelineV3Workspace::open_read_only(root)?;
    let value: serde_json::Value = local::read_bounded(&workspace.plan(), DOCUMENT_BYTES)?;
    PipelineV3Plan::from_json_slice(&canonical_json_bytes(&value)?).map_err(|e| invalid(e.message))
}
fn checkpoint(root: &Path, manifest: &PipelineV3RunManifest, id: &str) -> Result<StageCheckpoint> {
    let record = manifest
        .payload
        .stages
        .iter()
        .find(|s| s.stage_id == id)
        .ok_or_else(|| invalid("selected native spatial unit is not planned"))?;
    let reference = record
        .checkpoint
        .as_ref()
        .ok_or_else(|| invalid("selected spatial unit has no accepted checkpoint"))?;
    if record.state != PipelineV3StageState::Complete
        || record.rerun_required
        || record
            .excluded_checkpoints
            .contains(&reference.checkpoint_sha256)
    {
        return Err(invalid(
            "selected spatial unit is incomplete, excluded or requires rerun",
        ));
    }
    crate::load_stage_checkpoint(
        &crate::PipelineV3Workspace::open_read_only(root)?,
        reference,
    )
}
fn port<'a>(checkpoint: &'a StageCheckpoint, name: &str) -> Result<&'a ArtifactEvidence> {
    let values = checkpoint
        .payload
        .outputs
        .iter()
        .filter(|o| o.port == name)
        .collect::<Vec<_>>();
    if values.len() != 1 {
        return Err(invalid(
            "selected spatial output port is missing or ambiguous",
        ));
    }
    Ok(values[0])
}
fn path(root: &Path, artifact: &ArtifactEvidence) -> Result<PathBuf> {
    let path = root.join(
        artifact
            .relative_path
            .as_deref()
            .ok_or_else(|| invalid("accepted spatial output has no owned locator"))?,
    );
    if !path.starts_with(root) || path.canonicalize().ok().as_deref() != Some(path.as_path()) {
        return Err(invalid(
            "accepted spatial output escaped canonical native root",
        ));
    }
    Ok(path)
}
fn report<T: serde::de::DeserializeOwned>(
    root: &Path,
    checkpoint: &StageCheckpoint,
    maximum: u64,
) -> Result<(T, String)> {
    let artifact = port(checkpoint, "evidence")?;
    let path = path(root, artifact)?;
    if artifact.kind != ArtifactKind::File
        || identity(&path, maximum)? != (artifact.sha256.clone(), artifact.byte_count)
    {
        return Err(invalid("accepted spatial report bytes changed"));
    }
    Ok((
        local::read_bounded(&path, maximum)?,
        artifact.sha256.clone(),
    ))
}
fn origin(
    root: &Path,
    plan: &PipelineV3Plan,
    checkpoint: &StageCheckpoint,
    kind: SpatialOriginKind,
    report_sha256: String,
    decision: Option<String>,
) -> SpatialOrigin {
    SpatialOrigin {kind,run_directory:root.to_path_buf(),stage_id:checkpoint.payload.stage_id.clone(),plan_sha256:plan.plan_sha256.clone(),
        checkpoint_sha256:checkpoint.checkpoint_sha256.clone(),provider_lock_sha256:checkpoint.payload.provider_lock_sha256.clone(),report_sha256,
        import_decision_sha256:decision,processing_history:if kind==SpatialOriginKind::ReviewedHistoricalImport {"unknown; current import does not attest historical tools/models/settings"}else{"current native Upscayl checkpoint; exact locked settings retained; backend/creative qualification not established"}.into()}
}
fn spec(
    index: &RifeFrameIndex,
    processing_index_sha256: &str,
    frame: &UpscaylFrameSpec,
) -> Result<()> {
    let original = index
        .frames
        .get(frame.ordinal as usize)
        .ok_or_else(|| invalid("native spatial frame is outside original index"))?;
    let time = index
        .temporal_inspection
        .video_timeline()
        .map_err(|e| crate::Error::from_anyhow(crate::ErrorCategory::Media, e))?
        .frames[frame.ordinal as usize]
        .source_time;
    if frame.frame_index_sha256 != processing_index_sha256
        || frame.source_sha256 != index.temporal_inspection.source_sha256
        || frame.source_byte_count != index.temporal_inspection.source_size_bytes
        || frame.filename != original.filename
        || frame.input_sha256 != original.sha256
        || frame.source_time != time
        || frame.input_width != index.width
        || frame.input_height != index.height
        || frame.input_byte_count == 0
    {
        return Err(invalid(
            "native spatial unit differs from the exact original index/source/clock/input pixels",
        ));
    }
    Ok(())
}
fn locked(
    stage: &ResolvedPipelineStage,
    checkpoint: &StageCheckpoint,
    configuration: &crate::ProviderConfiguration,
    manifest: &crate::ProviderManifest,
) -> Result<()> {
    if !stage.depends_on.is_empty()
        || stage.provider_lock.lock_sha256 != checkpoint.payload.provider_lock_sha256
        || stage.provider_lock.payload.effective_configuration_sha256
            != configuration.effective_configuration_sha256
        || stage.provider_lock.payload.configuration_schema != configuration.configuration_schema
        || stage.provider_lock.payload.provider != configuration.provider
        || stage.capability.id != configuration.capability.id
        || stage.provider_lock.payload.manifest_sha256 != canonical_sha256(manifest)?
    {
        return Err(invalid(
            "native spatial report does not belong to this exact independent locked operation",
        ));
    }
    Ok(())
}
fn frame_record(
    request: &CarrierRequest,
    original: &crate::video_interpolation::RifeFrame,
    source_time: crate::temporal::TimeRange,
    file: PathBuf,
    sha: String,
    bytes: u64,
    decoded: String,
    origin_index: u32,
) -> Result<SelectedSpatialFrame> {
    if inspect_png(
        &file,
        request.output_width,
        request.output_height,
        request.maximum_frame_bytes,
    )? != (sha.clone(), bytes)
    {
        return Err(invalid(
            "native selected spatial PNG differs from accepted report",
        ));
    }
    Ok(SelectedSpatialFrame {
        ordinal: original.ordinal,
        filename: original.filename.clone(),
        sha256: sha,
        byte_count: bytes,
        decoded_rgb24_sha256: decoded,
        source_time,
        input_file: file,
        origin_index,
    })
}
pub(super) fn append(
    selection: &SpatialSelection,
    request: &CarrierRequest,
    index: &RifeFrameIndex,
    pin: &(String, u64),
    origins: &mut Vec<SpatialOrigin>,
    frames: &mut Vec<SelectedSpatialFrame>,
    token: &CancellationToken,
) -> Result<()> {
    match selection {
        SpatialSelection::Historical {
            run_directory,
            ordinals: selected,
        } => {
            ordinals(selected, 128)?;
            let report = historical::observe(run_directory, token)?;
            let before = crate::status_v3(run_directory)?;
            let plan = read_plan(run_directory)?;
            let cp = checkpoint(run_directory, &before, "historical_import")?;
            let subset = &report.subset;
            if subset.source_index_sha256 != pin.0
                || subset.source_sha256 != index.temporal_inspection.source_sha256
                || subset.original_frame_count != index.frames.len() as u64
                || subset.output_width != request.output_width
                || subset.output_height != request.output_height
            {
                return Err(invalid(
                    "historical spatial selection belongs to a different original index/clock/geometry",
                ));
            }
            let evidence = port(&cp, "evidence")?;
            let directory = path(run_directory, port(&cp, "frames")?)?;
            let origin_index = origins.len() as u32;
            origins.push(origin(
                run_directory,
                &plan,
                &cp,
                SpatialOriginKind::ReviewedHistoricalImport,
                evidence.sha256.clone(),
                Some(subset.import_decision_sha256.clone()),
            ));
            for ordinal in selected {
                let position = subset
                    .frames
                    .iter()
                    .position(|f| f.ordinal == *ordinal)
                    .ok_or_else(|| {
                        invalid("selected ordinal is absent from accepted historical subset")
                    })?;
                let f = &subset.frames[position];
                let original = index
                    .frames
                    .get(*ordinal as usize)
                    .ok_or_else(|| invalid("historical original ordinal missing"))?;
                frames.push(frame_record(
                    request,
                    original,
                    f.source_time,
                    directory.join(&f.filename),
                    f.sha256.clone(),
                    f.byte_count,
                    report.decoded_rgb24_sha256[position].clone(),
                    origin_index,
                )?);
            }
            if crate::status_v3(run_directory)? != before
                || historical::observe(run_directory, token)? != report
            {
                return Err(invalid(
                    "historical spatial acceptance changed during binding",
                ));
            }
        }
        SpatialSelection::Native {
            run_directory,
            configuration,
            units,
        } => {
            if units.is_empty() || units.len() > 1024 {
                return Err(invalid("select 1..1024 current native spatial units"));
            }
            let processing_index_sha256 = canonical_sha256(index)?;
            let dimensions = configuration.output_dimensions(index.width, index.height)?;
            if dimensions != (request.output_width, request.output_height) {
                return Err(invalid(
                    "native spatial output geometry differs from selected uniform carrier",
                ));
            }
            let before = crate::status_v3(run_directory)?;
            let plan = read_plan(run_directory)?;
            if !matches!(
                plan.payload.pipeline_name.as_str(),
                "upscayl-per-frame-recovery"
                    | "upscayl-bounded-chunk-recovery"
                    | "selected-upscayl-frame-recovery"
            ) || !plan.payload.inputs.iter().any(|p| {
                p.id == "source_index" && p.content_sha256 == pin.0 && p.byte_count == pin.1
            }) || !plan.payload.inputs.iter().any(|p| {
                p.id == "source_video"
                    && p.content_sha256 == index.temporal_inspection.source_sha256
                    && p.byte_count == index.temporal_inspection.source_size_bytes
            }) {
                return Err(invalid(
                    "native spatial source is not the fixed source/index-bound recovery profile",
                ));
            }
            let mut seen = BTreeSet::new();
            for unit in units {
                check_cancelled(token)?;
                ordinals(&unit.ordinals, 256)?;
                if !seen.insert(&unit.stage_id) {
                    return Err(invalid("duplicate selected native spatial unit"));
                }
                let stage = plan
                    .payload
                    .stages
                    .iter()
                    .find(|s| s.id == unit.stage_id)
                    .ok_or_else(|| invalid("selected native spatial stage missing"))?;
                let cp = checkpoint(run_directory, &before, &unit.stage_id)?;
                let origin_index = origins.len() as u32;
                if stage.capability.id == UPSCAYL_FRAME_CAPABILITY_ID {
                    let (report, report_sha256): (UpscaylFrameReport, String) =
                        report(run_directory, &cp, UPSCAYL_MAXIMUM_FRAME_REPORT_BYTES)?;
                    spec(index, &processing_index_sha256, &report.frame)?;
                    let operation_schema =
                        if report.schema == UPSCAYL_SELECTED_FRAME_REPORT_SCHEMA_V2 {
                            UPSCAYL_SELECTED_FRAME_OPERATION_SCHEMA_V2
                        } else {
                            UPSCAYL_FRAME_OPERATION_SCHEMA_V1
                        };
                    super::super::provider::verify_frame_report_for_operation(
                        &report,
                        index,
                        &processing_index_sha256,
                        report.frame.ordinal as usize,
                        configuration,
                        dimensions,
                        operation_schema,
                    )?;
                    let operation = UpscaylFrameOperation {
                        schema: operation_schema.into(),
                        settings: configuration.clone(),
                        frame: report.frame.clone(),
                    };
                    locked(
                        stage,
                        &cp,
                        &operation.provider_configuration()?,
                        &operation.manifest()?,
                    )?;
                    if unit.ordinals != vec![report.frame.ordinal]
                        || report.provider_lock_sha256 != stage.provider_lock.lock_sha256
                    {
                        return Err(invalid(
                            "native selected frame differs from exact checkpoint",
                        ));
                    }
                    let output = port(&cp, "frame")?;
                    if output.sha256 != report.output_sha256
                        || output.byte_count != report.output_byte_count
                    {
                        return Err(invalid("native frame/report identity mismatch"));
                    }
                    origins.push(origin(
                        run_directory,
                        &plan,
                        &cp,
                        SpatialOriginKind::NativeFrame,
                        report_sha256,
                        None,
                    ));
                    frames.push(frame_record(
                        request,
                        &index.frames[report.frame.ordinal as usize],
                        report.frame.source_time,
                        path(run_directory, output)?,
                        report.output_sha256,
                        report.output_byte_count,
                        report.output_decoded_rgb24_sha256,
                        origin_index,
                    )?);
                } else if stage.capability.id == UPSCAYL_CHUNK_CAPABILITY_ID {
                    let (report, report_sha256): (UpscaylChunkReport, String) =
                        report(run_directory, &cp, UPSCAYL_MAXIMUM_CHUNK_REPORT_BYTES)?;
                    for f in &report.frames {
                        spec(index, &processing_index_sha256, &f.frame)?;
                    }
                    let operation = UpscaylChunkOperation {
                        schema: UPSCAYL_CHUNK_OPERATION_SCHEMA_V1.into(),
                        settings: configuration.clone(),
                        chunk_ordinal: report.chunk_ordinal,
                        frames: report.frames.iter().map(|f| f.frame.clone()).collect(),
                    };
                    let config = operation.provider_configuration()?;
                    locked(stage, &cp, &config, &operation.manifest()?)?;
                    if report.schema != UPSCAYL_CHUNK_REPORT_SCHEMA_V1
                        || report.stage_id != stage.id
                        || report.stage_id != format!("upscale_chunk_{:06}", report.chunk_ordinal)
                        || report.provider_lock_sha256 != stage.provider_lock.lock_sha256
                        || report.configuration_sha256 != config.effective_configuration_sha256
                        || report.upscayl_sha256 != configuration.upscayl.sha256
                        || report.decoder_sha256 != configuration.import.ffmpeg.sha256
                        || report.model_param_sha256 != configuration.model.param.sha256
                        || report.model_weights_sha256 != configuration.model.weights.sha256
                        || report.native_scale != configuration.model.native_scale
                        || report.output_scale != configuration.output_scale
                        || (report.output_width, report.output_height) != dimensions
                        || report.backend_qualification != "unqualified"
                    {
                        return Err(invalid(
                            "native chunk report differs from exact source/model/scale/settings operation",
                        ));
                    }
                    let directory = path(run_directory, port(&cp, "frames")?)?;
                    origins.push(origin(
                        run_directory,
                        &plan,
                        &cp,
                        SpatialOriginKind::NativeChunk,
                        report_sha256,
                        None,
                    ));
                    for ordinal in &unit.ordinals {
                        let f = report
                            .frames
                            .iter()
                            .find(|f| f.frame.ordinal == *ordinal)
                            .ok_or_else(|| {
                                invalid("selected ordinal is not in accepted native chunk")
                            })?;
                        frames.push(frame_record(
                            request,
                            &index.frames[*ordinal as usize],
                            f.frame.source_time,
                            directory.join(&f.frame.filename),
                            f.output_sha256.clone(),
                            f.output_byte_count,
                            f.output_decoded_rgb24_sha256.clone(),
                            origin_index,
                        )?);
                    }
                } else {
                    return Err(invalid(
                        "only accepted Upscayl frame/chunk units can supply current native spatial work",
                    ));
                }
            }
            if crate::status_v3(run_directory)? != before {
                return Err(invalid(
                    "native spatial acceptance changed during selection",
                ));
            }
        }
    }
    Ok(())
}
