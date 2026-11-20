use super::*;
use crate::Result;
use crate::audio_inspection::AudioToolPin;
use crate::music_video_recipe::SourceMusicVideoConfiguration;
use crate::video_interpolation::RifeConfiguration;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|error| {
        resolve::configuration(format!(
            "cannot lower managed settings into the exact native contract: {error}"
        ))
    })
}
fn native_version(tool: &ResolvedManagedTool, provider: &str) -> String {
    if semver::Version::parse(&tool.version).is_ok() {
        tool.version.clone()
    } else {
        format!("0.0.0+{provider}.release.{}", tool.version)
    }
}
fn runtime(tool: &ResolvedManagedTool) -> Value {
    match (
        &tool.package_id,
        &tool.package_sha256,
        &tool.catalog_revision,
    ) {
        (Some(id), Some(digest), Some(revision)) => {
            json!({"package_id":id,"package_sha256":digest,"catalog_revision":revision,"runtime_files":tool.runtime_files,"installation_roots":tool.installation_roots})
        }
        _ => Value::Null,
    }
}
/// Pins local FFmpeg bytes without executing them. Native admission retains its
/// exact 8.0.1 banner gate; this declaration does not claim an observed probe.
pub fn local_ffmpeg_pin(path: Option<&Path>, name: &str) -> Result<AudioToolPin> {
    let path = match path {
        Some(path) => resolve::absolute(path.to_path_buf())?,
        None => find_on_path(name)?,
    };
    let (sha256, _) = resolve::hash_file(&path, true)?;
    Ok(AudioToolPin {
        executable: path,
        version: "8.0.1".to_owned(),
        sha256,
    })
}
fn find_on_path(name: &str) -> Result<PathBuf> {
    std::env::var_os("PATH").and_then(|value| std::env::split_paths(&value).map(|dir| dir.join(name)).find(|path| path.is_file())).map(resolve::absolute).transpose()?.ok_or_else(|| resolve::dependency(format!("{name} is unavailable on PATH; install compatible FFmpeg/ffprobe 8.0.1 or provide an explicit path")))
}
/// Lower into the established fixed extraction → import → Upscayl → optional
/// selected RIFE → assembly recipe. This does not run tools or create a run.
pub fn build_source(
    options: &ManagedMusicVideoConfiguration,
    upscayl: &ResolvedManagedTool,
    rife: Option<&ResolvedManagedTool>,
) -> Result<SourceMusicVideoConfiguration> {
    options.validate()?;
    let ffmpeg = local_ffmpeg_pin(options.ffmpeg.as_deref(), "ffmpeg")?;
    let ffprobe = local_ffmpeg_pin(options.ffprobe.as_deref(), "ffprobe")?;
    let import = json!({"schema":"aniflow.frame-import-configuration/v1","ffmpeg":ffmpeg,"maximum_source_bytes":options.maximum_source_bytes,"maximum_frame_bytes":options.maximum_input_frame_bytes,"maximum_total_frame_bytes":options.maximum_total_frame_bytes});
    let mut upscale = json!({"schema":"aniflow.upscayl-recovery-configuration/v1","import":import,"upscayl":{"executable":upscayl.executable,"version":native_version(upscayl,"upscayl"),"sha256":upscayl.executable_sha256},"model":{"name":upscayl.model_id,"native_scale":upscayl.model_scale,"param":{"path":upscayl.param_path,"sha256":upscayl.param_sha256},"weights":{"path":upscayl.weights_path,"sha256":upscayl.weights_sha256}},"output_scale":options.output_scale,"execution":{"mode":"bounded_chunk","frames_per_chunk":options.frames_per_chunk,"timeout_milliseconds":options.chunk_timeout_milliseconds},"maximum_working_bytes":options.maximum_working_bytes,"minimum_free_bytes":options.minimum_free_bytes,"maximum_output_frame_bytes":options.maximum_output_frame_bytes,"maximum_output_pixels":options.maximum_output_pixels});
    if !runtime(upscayl).is_null() {
        upscale["managed_runtime"] = runtime(upscayl);
    }
    let interpolation = match (&options.interpolation, rife) {
        (Some(settings), Some(tool)) => Some(build_rife(settings, tool)?),
        (None, None) => None,
        _ => {
            return Err(resolve::configuration(
                "selected RIFE must have exactly one resolved dependency set",
            ));
        }
    };
    let value = decode::<SourceMusicVideoConfiguration>(
        json!({"schema":"aniflow.source-music-video-recipe/v1","extraction":{"schema":"aniflow.frame-extraction-configuration/v1","ffmpeg":ffmpeg,"ffprobe":ffprobe,"source_selection":options.source_selection,"maximum_source_bytes":options.maximum_source_bytes,"maximum_capture_bytes":options.maximum_capture_bytes,"maximum_frames":options.maximum_frames,"maximum_frame_bytes":options.maximum_input_frame_bytes,"maximum_total_frame_bytes":options.maximum_total_frame_bytes,"maximum_working_bytes":options.maximum_working_bytes,"minimum_free_bytes":options.minimum_free_bytes},"recipe":{"schema":"aniflow.music-video-recipe/v1","repair":{"mode":"disabled","reason":"No frame repair selected in the managed source preset.","import":import},"upscale":{"mode":"upscayl","configuration":upscale},"assembly":{"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":ffmpeg,"ffprobe":ffprobe,"delivery_crf":options.delivery_crf,"maximum_output_bytes":options.maximum_video_output_bytes,"maximum_source_bytes":options.maximum_source_bytes,"maximum_capture_bytes":options.maximum_capture_bytes,"maximum_frame_bytes":options.maximum_output_frame_bytes,"maximum_total_frame_bytes":options.maximum_total_frame_bytes,"preset":options.preset,"maximum_working_bytes":options.maximum_working_bytes,"minimum_free_bytes":options.minimum_free_bytes},"interpolation_run":interpolation}}),
    )?;
    value.validate()?;
    Ok(value)
}

pub fn build_rife(
    options: &ManagedRifeConfiguration,
    tool: &ResolvedManagedTool,
) -> Result<RifeConfiguration> {
    options.validate()?;
    if tool
        .model_directory
        .file_name()
        .and_then(|value| value.to_str())
        != Some("rife-v4.6")
        || tool.param_path.file_name().and_then(|value| value.to_str()) != Some("flownet.param")
        || tool
            .weights_path
            .file_name()
            .and_then(|value| value.to_str())
            != Some("flownet.bin")
    {
        return Err(resolve::configuration(
            "the current RIFE profile requires rife-v4.6/flownet.param and flownet.bin",
        ));
    }
    let mut settings = json!({"tool":{"executable":tool.executable,"version":native_version(tool,"rife"),"sha256":tool.executable_sha256},"model":{"directory":tool.model_directory,"param_sha256":tool.param_sha256,"param_bytes":tool.param_bytes,"bin_sha256":tool.weights_sha256,"bin_bytes":tool.weights_bytes},"preset":options.preset,"device_id":options.device_id});
    if !runtime(tool).is_null() {
        settings["managed_runtime"] = runtime(tool);
    }
    let value = decode::<RifeConfiguration>(
        json!({"schema":"aniflow.rife-configuration/v1","enabled":true,"host":options.host,"settings":settings}),
    )?;
    value.validate()?;
    Ok(value)
}

/// Reuse the assembly owner's actual admission formula for observed inputs.
/// This is not an available-disk observation or an operating-system reservation.
pub fn source_assembly_working_bytes(
    configuration: &SourceMusicVideoConfiguration,
    source_bytes: u64,
    frame_bytes: u64,
    tool_bytes: u64,
) -> Result<u64> {
    crate::video_assembly::checked_working_bytes(
        &configuration.recipe.assembly,
        source_bytes,
        frame_bytes,
        tool_bytes,
    )
}

/// Observe the existing native validator's separate temporary-volume guard.
/// This is read-only and cannot reserve storage or prove later availability.
pub fn source_validator_storage_admission(
    configuration: &SourceMusicVideoConfiguration,
) -> Result<()> {
    crate::video_assembly::require_validator_storage(&configuration.recipe.assembly)
}
