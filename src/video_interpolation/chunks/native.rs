use super::*;
use crate::frame_io::{copy_pinned, inspect_png, inventory, require_absent};
use std::ffi::OsString;
fn binding<'a>(
    v: &'a [crate::ProviderInvocationArtifactBinding],
    port: &str,
    id: &str,
    kind: &str,
    directory: bool,
) -> Result<&'a Path> {
    let values = v
        .iter()
        .filter(|i| i.port == port && i.artifact_id == id)
        .collect::<Vec<_>>();
    if values.len() != 1 {
        return Err(invalid("chunk input/output binding absent or ambiguous"));
    }
    let a = values[0];
    if a.artifact_type != kind
        || a.kind
            != if directory {
                crate::ArtifactKind::Directory
            } else {
                crate::ArtifactKind::File
            }
        || a.artifact_role
            != if directory {
                crate::ArtifactRole::Intermediate
            } else {
                crate::ArtifactRole::ValidationEvidence
            }
        || a.stream_role
            != Some(if directory {
                crate::StreamRole::Video
            } else {
                crate::StreamRole::TimedMetadata
            })
    {
        return Err(invalid("chunk artifact type/role/stream/kind differs"));
    }
    Ok(&a.path)
}
pub(super) fn inspect_complete(
    dir: &Path,
    input: &RecoveredInput,
    c: &RifeConfiguration,
    out: &RifeOutputIndex,
) -> Result<()> {
    recovered::validate_output(out, c, &input.carrier)?;
    let width = input.carrier.selection.request.output_width;
    let height = input.carrier.selection.request.output_height;
    let max = super::super::provider::frame_byte_bound_dimensions(width, height, &c.settings)?;
    inventory(
        dir,
        &out.frames
            .iter()
            .map(|f| OsString::from(&f.filename))
            .collect::<Vec<_>>(),
    )?;
    for f in &out.frames {
        if inspect_png(&dir.join(&f.filename), width, height, max)?.0 != f.sha256 {
            return Err(invalid("complete chunked RIFE output PNG changed"));
        }
    }
    Ok(())
}
pub(crate) fn execute_invocation(inv: &crate::ProviderInvocationRequest) -> Result<()> {
    inv.validate()?;
    let op: Operation = serde_json::from_value(
        serde_json::to_value(&inv.configuration.values)
            .map_err(|_| invalid("chunk operation encoding"))?,
    )
    .map_err(|_| invalid("closed RIFE chunk operation required"))?;
    op.validate()?;
    if inv.configuration != op.configuration()? || !cfg!(unix) {
        return Err(invalid("chunk native configuration/platform differs"));
    }
    let token = CancellationToken::new();
    let input = RecoveredInput::observe(&op.request.base.source, &token)?;
    let full = recovered::plan(&op.request.base.configuration, &input.carrier)?;
    let parts = ranges(full.output_frame_count, op.request.output_frames_per_chunk)?;
    let index = binding(
        &inv.inputs,
        "index",
        "source_index",
        recovered::INPUT_TYPE,
        false,
    )?;
    let current: crate::upscayl_recovery::carrier::CompleteSpatialCarrier =
        local::read_bounded(index, 67_108_864)?;
    if current != input.carrier
        || identity(index, 67_108_864)?.0 != op.request.base.source.carrier_sha256
    {
        return Err(invalid("chunk original carrier snapshot differs"));
    }
    if let Role::Chunk {
        first_output,
        end_output,
    } = op.role
    {
        let n = parts
            .iter()
            .position(|p| *p == (first_output, end_output))
            .ok_or_else(|| invalid("chunk is not an exact selected complete-set range"))?;
        if inv.stage_id != id(n) || inv.inputs.len() != 2 || inv.outputs.len() != 2 {
            return Err(invalid(
                "chunk invocation differs from fixed native range stage",
            ));
        }
        binding(
            &inv.inputs,
            "frames",
            "source_frames",
            "application/vnd.aniflow.rife-frames",
            true,
        )?;
        binding(
            &inv.outputs,
            "frames",
            &format!("chunk_{n:04}_frames"),
            "application/vnd.aniflow.rife-frames",
            true,
        )?;
        binding(
            &inv.outputs,
            "index",
            &format!("chunk_{n:04}_index"),
            CHUNK_TYPE,
            false,
        )?;
        let frames = input.carrier.frames();
        let width = input.carrier.selection.request.output_width;
        let height = input.carrier.selection.request.output_height;
        let bound = super::super::provider::spatial::BoundRender {
            width,
            height,
            source_frames_spec: &frames,
            interpolation: full,
            output_schema: CHUNK_SCHEMA,
            index_sha256: &op.request.base.source.carrier_sha256,
            upstream: &op.request.base.source.carrier_directory,
        };
        return super::super::provider::spatial::render_bound_range(
            inv,
            &op.request.base.configuration,
            bound,
            Some((first_output, end_output)),
            |output| {
                canonical_json_bytes(&ChunkReport {
                    schema: "aniflow.rife-chunk-report/v1".into(),
                    operation: op.clone(),
                    provider_lock_sha256: inv.provider_lock_sha256.clone(),
                    output: output.clone(),
                })
            },
            |out| chunk_output(&op, &input, out),
            || {
                if RecoveredInput::observe(&op.request.base.source, &token)? != input {
                    return Err(invalid("recovered RIFE source changed during chunk"));
                }
                Ok(())
            },
        );
    }
    if inv.stage_id != "collect_rife_chunks"
        || inv.inputs.len() != 1 + parts.len() * 2
        || inv.outputs.len() != 2
    {
        return Err(invalid("chunk collector requires exact complete input set"));
    }
    let frames = binding(
        &inv.outputs,
        "frames",
        "interpolated_frames",
        "application/vnd.aniflow.rife-frames",
        true,
    )?;
    let output = binding(
        &inv.outputs,
        "index",
        "interpolated_index",
        recovered::OUTPUT_TYPE,
        false,
    )?;
    let parent = frames
        .parent()
        .ok_or_else(|| invalid("chunk collector parent missing"))?;
    if output.parent() != Some(parent) || frames == output {
        return Err(invalid("collector outputs must be distinct siblings"));
    }
    crate::frame_io::ensure_parent(parent)?;
    require_absent(frames)?;
    require_absent(output)?;
    for destination in [frames, output] {
        for source in inv.inputs.iter().map(|i| i.path.as_path()).chain([
            op.request.base.source.carrier_directory.as_path(),
            op.request
                .base
                .configuration
                .settings
                .tool
                .executable
                .as_path(),
            op.request
                .base
                .configuration
                .settings
                .model
                .directory
                .as_path(),
        ]) {
            if source.starts_with(destination) || destination.starts_with(source) {
                return Err(invalid("collector output overlaps input/components"));
            }
        }
    }
    let width = input.carrier.selection.request.output_width;
    let height = input.carrier.selection.request.output_height;
    let maximum = super::super::provider::frame_byte_bound_dimensions(
        width,
        height,
        &op.request.base.configuration.settings,
    )?;
    let needed = maximum
        .checked_mul(full.output_frame_count)
        .and_then(|v| v.checked_mul(2))
        .and_then(|v| v.checked_add(RIFE_MAXIMUM_INDEX_BYTES.checked_mul(2)?))
        .ok_or_else(|| invalid("chunk collector working bound overflow"))?;
    if needed
        > op.request
            .base
            .configuration
            .settings
            .maximum_artifact_bytes
    {
        return Err(invalid("chunk collection exceeds selected admission"));
    }
    crate::video_assembly::require_storage(parent, needed, 0)?;
    let temporary = tempfile::Builder::new()
        .prefix(".rife-chunk-collection-")
        .tempdir_in(parent)
        .map_err(|_| invalid("cannot reserve private chunk collector"))?;
    let private = temporary
        .path()
        .canonicalize()
        .map_err(|_| invalid("cannot resolve chunk collector"))?;
    let result: Result<()> = (|| {
        let destination = private.join("frames");
        local::directory(&destination)?;
        let mut out = RifeOutputIndex {
            schema: recovered::OUTPUT_SCHEMA.into(),
            plan: full,
            frames: Vec::new(),
        };
        let mut dependencies = Vec::new();
        for (n, (first, end)) in parts.iter().copied().enumerate() {
            let source = binding(
                &inv.inputs,
                "frames",
                &format!("chunk_{n:04}_frames"),
                "application/vnd.aniflow.rife-frames",
                true,
            )?;
            let record = binding(
                &inv.inputs,
                "chunks",
                &format!("chunk_{n:04}_index"),
                CHUNK_TYPE,
                false,
            )?;
            let report_pin = identity(record, RIFE_MAXIMUM_INDEX_BYTES)?;
            let report: ChunkReport = local::read_bounded(record, RIFE_MAXIMUM_INDEX_BYTES)?;
            let expected = Operation {
                schema: "aniflow.rife-chunk-operation/v1".into(),
                request: op.request.clone(),
                role: Role::Chunk {
                    first_output: first,
                    end_output: end,
                },
            };
            if report.schema != "aniflow.rife-chunk-report/v1" || report.operation != expected {
                return Err(invalid(
                    "chunk report belongs to a different selected range/source/settings",
                ));
            }
            require_sha256(&report.provider_lock_sha256, "chunk producer lock")?;
            chunk_output(&expected, &input, &report.output)?;
            // Native v3 preflight fixes the dependency topology/locks. These
            // records supplement that actual authority, never replace it.
            inventory(
                source,
                &report
                    .output
                    .frames
                    .iter()
                    .map(|f| OsString::from(&f.filename))
                    .collect::<Vec<_>>(),
            )?;
            for frame in report.output.frames {
                let p = source.join(&frame.filename);
                let pin = inspect_png(&p, width, height, maximum)?;
                if pin.0 != frame.sha256 {
                    return Err(invalid("accepted chunk PNG differs from exact report"));
                }
                copy_pinned(
                    &p,
                    &destination.join(&frame.filename),
                    &pin,
                    maximum,
                    &token,
                )?;
                dependencies.push((p, pin, maximum));
                out.frames.push(frame);
            }
            dependencies.push((record.to_path_buf(), report_pin, RIFE_MAXIMUM_INDEX_BYTES));
        }
        inspect_complete(&destination, &input, &op.request.base.configuration, &out)?;
        for (path, pin, bound) in dependencies {
            if identity(&path, bound)? != pin {
                return Err(invalid("chunk dependency changed during collection"));
            }
        }
        if identity(index, 67_108_864)?.0 != op.request.base.source.carrier_sha256
            || RecoveredInput::observe(&op.request.base.source, &token)? != input
        {
            return Err(invalid(
                "complete source carrier/origins changed during collection",
            ));
        }
        let bytes = canonical_json_bytes(&out)?;
        if bytes.len() as u64 > RIFE_MAXIMUM_INDEX_BYTES {
            return Err(invalid("complete RIFE index exceeds bound"));
        }
        std::fs::rename(&destination, frames)
            .map_err(|_| invalid("cannot publish collected RIFE frames"))?;
        // Large output indexes use the existing no-clobber native publisher.
        super::super::provider::publish_json(output, &bytes)?;
        local::sync(parent)
    })();
    if result.is_err() {
        let _retained = temporary.keep();
    }
    result
}
