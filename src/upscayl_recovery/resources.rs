//! Measured input/component bytes and the exact conservative admission budget.
//!
//! This observer hashes local files, checks PNG structure and reads available
//! storage. It never launches a provider, reserves space, weakens policy or
//! qualifies a GPU. Native tensor/device working sets must be measured separately.
use super::*;
use crate::frame_io::{ensure_canonical_directory, inspect_png, inventory};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;

pub const RESOURCE_REPORT_SCHEMA: &str = "aniflow.upscayl-resource-report/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PngMeasurements {
    pub frame_count: u64,
    pub total_bytes: u64,
    pub smallest_bytes: u64,
    pub largest_bytes: u64,
    /// Ordered filename/hash/length binding; not evidence of complete decode.
    pub inventory_sha256: String,
    pub complete_decode_performed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentMeasurement {
    pub role: String,
    pub sha256: String,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryDiskBudget {
    pub frame_count: u64,
    pub checkpoint_units: u64,
    pub maximum_frames_for_selected_partition: u64,
    pub frames_per_unit: u64,
    pub original_source_bytes: u64,
    pub original_index_bytes: u64,
    pub original_png_bytes: u64,
    pub external_original_component_bytes: u64,
    pub output_candidates_and_accepted_copies_bound_bytes: u64,
    pub append_only_metadata_bound_bytes: u64,
    pub simultaneous_component_copies_bound_bytes: u64,
    pub active_png_scratch_bound_bytes: u64,
    pub shared_reports_bound_bytes: u64,
    /// New free space required for a fresh run, excluding already-present inputs.
    pub additional_disk_bound_bytes: u64,
    /// Admission footprint includes source/index/PNGs, excludes external cache.
    pub working_bound_bytes: u64,
    pub maximum_working_bytes: u64,
    pub fits_working_policy: bool,
    pub minimum_free_bytes: u64,
    pub available_bytes_observed: Option<u64>,
    pub required_free_including_margin_bytes: u64,
    pub fits_available_storage: Option<bool>,
    pub reservation_or_quota: bool,
    pub retry_scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryGeometryCosts {
    pub input_width: u32,
    pub input_height: u32,
    pub native_width: u32,
    pub native_height: u32,
    pub requested_width: u32,
    pub requested_height: u32,
    pub input_rgb24_bytes_per_image: u64,
    pub native_rgb24_bytes_per_image: u64,
    pub native_float32_rgb_bytes_per_image: u64,
    pub requested_rgb24_bytes_per_image: u64,
    pub measured_device_or_rss_bytes: Option<u64>,
    pub native_intermediate_png_created_by_adapter: bool,
    pub interpretation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryResourceReport {
    pub schema: String,
    pub configuration_sha256: String,
    pub source_sha256: String,
    pub frame_index_sha256: String,
    pub input_pngs: PngMeasurements,
    pub components: Vec<ComponentMeasurement>,
    pub managed_runtime: Option<crate::toolchain::managed_runtime::ManagedRuntimePin>,
    pub disk: RecoveryDiskBudget,
    pub geometry: RecoveryGeometryCosts,
    pub retained_workspace: Option<RetainedWorkspaceMeasurement>,
    pub estimated_remaining_milliseconds: Option<u64>,
    pub throughput_basis: String,
    pub native_qualified: bool,
    pub output_reuse_authority: String,
}

fn sum(values: &[u64]) -> Result<u64> {
    values.iter().try_fold(0_u64, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| invalid("Upscayl resource byte sum overflow"))
    })
}
fn mul(left: u64, right: u64) -> Result<u64> {
    left.checked_mul(right)
        .ok_or_else(|| invalid("Upscayl resource byte product overflow"))
}

pub(super) fn disk_budget(
    settings: &UpscaylRecoveryConfiguration,
    frame_count: u64,
    source_bytes: u64,
    index_bytes: u64,
    frame_bytes: u64,
    component_bytes: u64,
) -> Result<RecoveryDiskBudget> {
    settings.validate()?;
    let (frames_per_unit, maximum_frames) = match settings.execution {
        UpscaylExecutionPolicy::PerFrame => (1, UPSCAYL_MAXIMUM_FRAMES as u64),
        UpscaylExecutionPolicy::BoundedChunk {
            frames_per_chunk, ..
        } => {
            let size = u64::from(frames_per_chunk);
            (
                size,
                mul(size, UPSCAYL_MAXIMUM_CHUNKS as u64)?.min(UPSCAYL_MAXIMUM_CHUNK_FRAMES as u64),
            )
        }
    };
    if frame_count == 0 || frame_count > maximum_frames {
        return Err(invalid(format!(
            "selected Upscayl partition admits 1..={maximum_frames} frames; do not bypass admission by lowering output bounds"
        )));
    }
    let units = frame_count.div_ceil(frames_per_unit);
    let output_copies = mul(mul(frame_count, settings.maximum_output_frame_bytes)?, 3)?;
    let metadata = mul(mul(units + 1, sum(&[frame_count, units, 1])?)?, 16_384)?;
    // Per-frame's original bound is (N+1)^2. Keep its exact conservative
    // allowance rather than accidentally widening previously admitted storage.
    let metadata = if settings.execution.is_per_frame() {
        mul(mul(frame_count + 1, frame_count + 1)?, 16_384)?
    } else {
        metadata
    };
    let component_copies = mul(component_bytes, 2)?;
    let scratch = mul(
        frames_per_unit,
        sum(&[
            settings.import.maximum_frame_bytes,
            settings.maximum_output_frame_bytes,
        ])?,
    )?;
    let reports = mul(4, UPSCAYL_MAXIMUM_REPORT_BYTES)?;
    let additional = sum(&[output_copies, metadata, component_copies, scratch, reports])?;
    let working = sum(&[additional, source_bytes, index_bytes, frame_bytes])?;
    Ok(RecoveryDiskBudget {
        frame_count,checkpoint_units:units,maximum_frames_for_selected_partition:maximum_frames,frames_per_unit,
        original_source_bytes:source_bytes,original_index_bytes:index_bytes,original_png_bytes:frame_bytes,
        external_original_component_bytes:component_bytes,
        output_candidates_and_accepted_copies_bound_bytes:output_copies,append_only_metadata_bound_bytes:metadata,
        simultaneous_component_copies_bound_bytes:component_copies,active_png_scratch_bound_bytes:scratch,
        shared_reports_bound_bytes:reports,additional_disk_bound_bytes:additional,working_bound_bytes:working,
        maximum_working_bytes:settings.maximum_working_bytes,fits_working_policy:working<=settings.maximum_working_bytes,
        minimum_free_bytes:settings.minimum_free_bytes,available_bytes_observed:None,
        required_free_including_margin_bytes:sum(&[additional,settings.minimum_free_bytes])?,fits_available_storage:None,
        reservation_or_quota:false,
        retry_scope:"fresh serial run with bounded active scratch and one retained failed component snapshot; repeated explicit failures can retain additional scratch without a finite lifetime bound. Observe existing occupancy and next-operation free space on each resume; no automatic retry or deletion".into(),
    })
}

fn png_measurements(
    directory: &Path,
    index: &crate::video_interpolation::RifeFrameIndex,
    width: u32,
    height: u32,
    maximum: u64,
    expected_source_hashes: bool,
    token: &CancellationToken,
) -> Result<PngMeasurements> {
    let names: Vec<OsString> = index
        .frames
        .iter()
        .map(|frame| frame.filename.clone().into())
        .collect();
    inventory(directory, &names)?;
    let mut sizes = Vec::with_capacity(index.frames.len());
    let mut pins = Vec::with_capacity(index.frames.len());
    for frame in &index.frames {
        check_cancelled(token)?;
        let path = directory.join(&frame.filename);
        let pin = inspect_png(&path, width, height, maximum)?;
        if expected_source_hashes && pin.0 != frame.sha256 {
            return Err(invalid(
                "resource observation source PNG differs from its exact index",
            ));
        }
        sizes.push(pin.1);
        pins.push((frame.filename.clone(), pin));
    }
    // Do not silently join measurements from two file generations.
    for (name, pin) in &pins {
        check_cancelled(token)?;
        if identity(&directory.join(name), maximum)? != *pin {
            return Err(invalid("PNG changed during resource observation"));
        }
    }
    inventory(directory, &names)?;
    Ok(PngMeasurements {
        frame_count: sizes.len() as u64,
        total_bytes: sum(&sizes)?,
        smallest_bytes: sizes.iter().copied().min().unwrap_or(0),
        largest_bytes: sizes.iter().copied().max().unwrap_or(0),
        inventory_sha256: canonical_sha256(&pins)?,
        complete_decode_performed: false,
    })
}

/// Measure an exact complete output directory as an unaccepted PNG sample.
/// The ordinary provider/runtime still owns complete decode and acceptance.
pub fn measure_output_pngs(
    request: &UpscaylRecoveryRequest,
    directory: &Path,
    token: &CancellationToken,
) -> Result<PngMeasurements> {
    request.configuration.validate()?;
    let (index, pin) = frame_import::read_index(&request.frame_index)?;
    let (width, height) = request
        .configuration
        .output_dimensions(index.width, index.height)?;
    let observed = png_measurements(
        directory,
        &index,
        width,
        height,
        request.configuration.maximum_output_frame_bytes,
        false,
        token,
    )?;
    if identity(
        &request.frame_index,
        crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES,
    )? != pin
    {
        return Err(invalid("index changed during output measurement"));
    }
    Ok(observed)
}

/// Explain admission before launch, including an over-policy result. The parent
/// must already exist so the storage observation is on the intended filesystem.
pub fn observe(
    request: &UpscaylRecoveryRequest,
    storage_parent: &Path,
    token: &CancellationToken,
) -> Result<RecoveryResourceReport> {
    check_cancelled(token)?;
    request.configuration.validate()?;
    request.execution_limits.validate()?;
    ensure_canonical_directory(storage_parent)?;
    let admitted = frame_import::prepare_inputs(&super::import_request(request), token)?;
    let settings = &request.configuration;
    let component_total = super::verify_components(settings, token)?;
    let input_pngs = png_measurements(
        &request.frames_directory,
        &admitted.index,
        admitted.index.width,
        admitted.index.height,
        settings.import.maximum_frame_bytes,
        true,
        token,
    )?;
    if input_pngs.total_bytes != admitted.frame_bytes {
        return Err(invalid("resource input byte generations differ"));
    }
    let mut components = Vec::new();
    let mut component_pins = Vec::new();
    for (role, path, max, expected_sha256) in [
        (
            "processor",
            &settings.upscayl.executable,
            MAXIMUM_TOOL_BYTES,
            &settings.upscayl.sha256,
        ),
        (
            "decoder",
            &settings.import.ffmpeg.executable,
            MAXIMUM_TOOL_BYTES,
            &settings.import.ffmpeg.sha256,
        ),
        (
            "model_param",
            &settings.model.param.path,
            MAXIMUM_MODEL_BYTES,
            &settings.model.param.sha256,
        ),
        (
            "model_weights",
            &settings.model.weights.path,
            MAXIMUM_MODEL_BYTES,
            &settings.model.weights.sha256,
        ),
    ] {
        let pin = identity(path, max)?;
        if pin.0 != *expected_sha256 {
            return Err(invalid(
                "resource component differs from its configured hash",
            ));
        }
        component_pins.push((path.clone(), pin.clone(), max));
        components.push(ComponentMeasurement {
            role: role.into(),
            sha256: pin.0,
            byte_count: pin.1,
        });
    }
    if let Some(runtime) = &settings.managed_runtime {
        for file in &runtime.runtime_files {
            let pin = identity(&file.path, 134_217_728)?;
            if pin != (file.sha256.clone(), file.byte_count) {
                return Err(invalid(
                    "resource runtime file differs from its configured pin",
                ));
            }
            component_pins.push((file.path.clone(), pin.clone(), 134_217_728));
            components.push(ComponentMeasurement {
                role: format!("runtime:{}", file.relative_path.display()),
                sha256: pin.0,
                byte_count: pin.1,
            });
        }
    }
    if sum(&components.iter().map(|c| c.byte_count).collect::<Vec<_>>())? != component_total
        || super::verify_components(settings, token)? != component_total
    {
        return Err(invalid("resource component generations differ"));
    }
    let mut disk = disk_budget(
        settings,
        admitted.index.frames.len() as u64,
        admitted.source.1,
        admitted.raw_index.1,
        admitted.frame_bytes,
        component_total,
    )?;
    let available = crate::cache_v3::available_storage(storage_parent)?;
    disk.available_bytes_observed = Some(available);
    disk.fits_available_storage = Some(disk.required_free_including_margin_bytes <= available);
    let (width, height) =
        settings.output_dimensions(admitted.index.width, admitted.index.height)?;
    let native_width = admitted
        .index
        .width
        .checked_mul(u32::from(settings.model.native_scale))
        .ok_or_else(|| invalid("native width overflow"))?;
    let native_height = admitted
        .index
        .height
        .checked_mul(u32::from(settings.model.native_scale))
        .ok_or_else(|| invalid("native height overflow"))?;
    let native_pixels = mul(u64::from(native_width), u64::from(native_height))?;
    let geometry=RecoveryGeometryCosts {input_width:admitted.index.width,input_height:admitted.index.height,native_width,native_height,requested_width:width,requested_height:height,input_rgb24_bytes_per_image:mul(mul(u64::from(admitted.index.width),u64::from(admitted.index.height))?,3)?,native_rgb24_bytes_per_image:mul(native_pixels,3)?,native_float32_rgb_bytes_per_image:mul(native_pixels,12)?,requested_rgb24_bytes_per_image:mul(mul(u64::from(width),u64::from(height))?,3)?,measured_device_or_rss_bytes:None,native_intermediate_png_created_by_adapter:false,interpretation:"geometry-only single-image lower bounds for RGB24 and float32 RGB representations; excludes NCNN tensors, tiles, model/device allocations, driver and concurrent host memory. Native 4x remains relevant for requested 2x/3x; the adapter creates no separate native4x PNG. These values are neither measured RSS/VRAM nor GPU quotas".into()};
    let after = frame_import::prepare_inputs(&super::import_request(request), token)?;
    if admitted.source != after.source
        || admitted.raw_index != after.raw_index
        || admitted.index != after.index
        || admitted.frame_bytes != after.frame_bytes
    {
        return Err(invalid(
            "source/index/PNG generation changed during resource observation",
        ));
    }
    for (path, pin, maximum) in &component_pins {
        check_cancelled(token)?;
        if identity(path, *maximum)? != *pin {
            return Err(invalid("resource component changed during observation"));
        }
    }
    check_cancelled(token)?;
    Ok(RecoveryResourceReport{schema:RESOURCE_REPORT_SCHEMA.into(),configuration_sha256:canonical_sha256(settings)?,source_sha256:admitted.source.0,frame_index_sha256:canonical_sha256(&admitted.index)?,input_pngs,components,managed_runtime:settings.managed_runtime.clone(),disk,geometry,retained_workspace:None,estimated_remaining_milliseconds:None,throughput_basis:"unavailable before accepted measured provider durations; recovery progress can report a conditional serial-work estimate, never a completion guarantee".into(),native_qualified:false,output_reuse_authority:"none; resource measurements do not authorize processing or checkpoint reuse".into()})
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedWorkspaceMeasurement {
    pub inventory_sha256: String,
    pub regular_file_count: u64,
    pub directory_count: u64,
    pub logical_file_bytes: u64,
    pub private_scratch_roots: u64,
    pub private_scratch_logical_file_bytes: u64,
    pub available_bytes_observed: u64,
    pub allocated_blocks_or_device_bytes_measured: bool,
    pub interpretation: String,
}

type LengthInventory = Vec<(PathBuf, bool, u64, Option<std::time::SystemTime>)>;
fn retained_lengths(root: &Path, token: &CancellationToken) -> Result<LengthInventory> {
    ensure_canonical_directory(root)?;
    let mut rows = Vec::new();
    let mut total = 0_u64;
    let mut path_bytes = 0_usize;
    for entry in walkdir::WalkDir::new(root)
        .min_depth(1)
        .max_depth(64)
        .follow_links(false)
    {
        check_cancelled(token)?;
        let entry = entry.map_err(|_| invalid("cannot observe retained workspace occupancy"))?;
        if rows.len() >= 1_000_000 {
            return Err(invalid("retained occupancy exceeds one million entries"));
        }
        let kind = entry.file_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(invalid(
                "retained occupancy refuses symlinks and special files",
            ));
        }
        let metadata = entry
            .metadata()
            .map_err(|_| invalid("cannot inspect retained file length"))?;
        let bytes = if kind.is_file() { metadata.len() } else { 0 };
        total = sum(&[total, bytes])?;
        if total > 17_592_186_044_416 {
            return Err(invalid("retained occupancy exceeds16TiB"));
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| invalid("retained occupancy escaped root"))?
            .to_path_buf();
        path_bytes = path_bytes
            .checked_add(
                relative
                    .to_str()
                    .ok_or_else(|| invalid("retained occupancy requires UTF-8 paths"))?
                    .len(),
            )
            .filter(|n| *n <= 67_108_864)
            .ok_or_else(|| invalid("retained occupancy path inventory exceeds64MiB"))?;
        if relative.components().count() >= 64 && kind.is_dir() {
            return Err(invalid("retained occupancy nesting exceeds its bound"));
        }
        rows.push((relative, kind.is_dir(), bytes, metadata.modified().ok()));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(rows)
}

/// Observe existing logical occupancy, including retained private attempts.
/// Two equal metadata inventories detect ordinary concurrent changes; they are
/// not a lock, block-allocation measurement, hash/decode check or reuse proof.
pub fn measure_retained_workspace(
    root: &Path,
    token: &CancellationToken,
) -> Result<RetainedWorkspaceMeasurement> {
    let rows = retained_lengths(root, token)?;
    let mut scratch_roots = std::collections::BTreeSet::new();
    let mut scratch_bytes = 0_u64;
    let mut total = 0_u64;
    let mut files = 0_u64;
    let mut directories = 0_u64;
    for (path, is_directory, bytes, _) in &rows {
        total = sum(&[total, *bytes])?;
        if *is_directory {
            directories += 1;
        } else {
            files += 1;
        }
        let mut ancestor = PathBuf::new();
        for component in path.components() {
            ancestor.push(component);
            if component.as_os_str().to_str().is_some_and(|n| {
                n.starts_with(".upscayl-frame-") || n.starts_with(".upscayl-chunk-")
            }) {
                scratch_roots.insert(ancestor.clone());
                scratch_bytes = sum(&[scratch_bytes, *bytes])?;
                break;
            }
        }
    }
    if retained_lengths(root, token)? != rows {
        return Err(invalid(
            "retained workspace changed during occupancy observation",
        ));
    }
    let mut digest = Sha256::new();
    for (path, is_directory, bytes, modified) in &rows {
        let path = path
            .to_str()
            .ok_or_else(|| invalid("retained occupancy requires UTF-8 paths"))?;
        digest.update((path.len() as u64).to_le_bytes());
        digest.update(path.as_bytes());
        digest.update([u8::from(*is_directory)]);
        digest.update(bytes.to_le_bytes());
        if let Some(time) = modified.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) {
            digest.update(time.as_nanos().to_le_bytes());
        }
    }
    Ok(RetainedWorkspaceMeasurement{inventory_sha256:format!("{:x}",digest.finalize()),regular_file_count:files,directory_count:directories,logical_file_bytes:total,private_scratch_roots:scratch_roots.len() as u64,private_scratch_logical_file_bytes:scratch_bytes,available_bytes_observed:crate::cache_v3::available_storage(root)?,allocated_blocks_or_device_bytes_measured:false,interpretation:"metadata length inventory only, observed twice without locking; includes private scratch from failed or possibly active attempts. Not allocated filesystem blocks, full hashes/decode, checkpoint acceptance or a lifetime retry bound. Shared available space may change immediately".into()})
}
