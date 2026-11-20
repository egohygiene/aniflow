//! Thin delivery for managed dependency resolution; media remains library-owned.
use super::*;
use aniflow::managed_experience::{
    self as managed, ManagedMusicVideoConfiguration, ManagedOptions, ManagedRifeConfiguration,
    ManagedToolOptions,
};
use aniflow::toolchain::managed::{
    HttpsDownloader, ManagedControl, ManagedProvider, bundled_catalog,
};

#[derive(Debug, Clone, Default, Args)]
pub(super) struct ManagedArguments {
    /// Optional closed user defaults; project config and flags take precedence.
    #[arg(long)]
    managed_user_configuration: Option<PathBuf>,
    /// Self-contained verified binary/model cache; never the source/run directory.
    #[arg(long)]
    managed_cache_directory: Option<PathBuf>,
    /// Refuse network; verified cached artifacts may be installed locally.
    #[arg(long)]
    offline: bool,
    /// Refuse downloads, allowing only installed or verified cached bytes.
    #[arg(long)]
    no_download: bool,
    #[arg(long)]
    upscayl_executable: Option<PathBuf>,
    #[arg(long)]
    upscayl_model_directory: Option<PathBuf>,
    #[arg(long)]
    upscayl_model_param: Option<PathBuf>,
    #[arg(long)]
    upscayl_model_weights: Option<PathBuf>,
    /// Upstream managed release ID, or explicit manual executable version.
    #[arg(long)]
    upscayl_version: Option<String>,
    #[arg(long)]
    upscayl_model: Option<String>,
    #[arg(long)]
    rife_executable: Option<PathBuf>,
    #[arg(long)]
    rife_model_directory: Option<PathBuf>,
    #[arg(long)]
    rife_model_param: Option<PathBuf>,
    #[arg(long)]
    rife_model_weights: Option<PathBuf>,
    #[arg(long)]
    rife_version: Option<String>,
    #[arg(long)]
    rife_model: Option<String>,
}
impl ManagedArguments {
    pub(super) fn present(&self) -> bool {
        self.managed_user_configuration.is_some()
            || self.managed_cache_directory.is_some()
            || self.offline
            || self.no_download
            || self.upscayl_executable.is_some()
            || self.upscayl_model_directory.is_some()
            || self.upscayl_model_param.is_some()
            || self.upscayl_model_weights.is_some()
            || self.upscayl_version.is_some()
            || self.upscayl_model.is_some()
            || self.rife_executable.is_some()
            || self.rife_model_directory.is_some()
            || self.rife_model_param.is_some()
            || self.rife_model_weights.is_some()
            || self.rife_version.is_some()
            || self.rife_model.is_some()
    }
    fn overrides_present(&self) -> bool {
        let mut selected = self.clone();
        selected.offline = false;
        selected.no_download = false;
        selected.present()
    }
    pub(super) fn effective(&self, project: &ManagedOptions) -> Result<ManagedOptions> {
        let user = self
            .managed_user_configuration
            .as_ref()
            .map(managed::load_user_configuration)
            .transpose()?
            .unwrap_or_default();
        let command = ManagedOptions {
            cache_directory: self.managed_cache_directory.clone(),
            offline: self.offline,
            no_download: self.no_download,
            upscayl: ManagedToolOptions {
                executable: self.upscayl_executable.clone(),
                model_directory: self.upscayl_model_directory.clone(),
                model_param: self.upscayl_model_param.clone(),
                model_weights: self.upscayl_model_weights.clone(),
                version: self.upscayl_version.clone(),
                model: self.upscayl_model.clone(),
            },
            rife: ManagedToolOptions {
                executable: self.rife_executable.clone(),
                model_directory: self.rife_model_directory.clone(),
                model_param: self.rife_model_param.clone(),
                model_weights: self.rife_model_weights.clone(),
                version: self.rife_version.clone(),
                model: self.rife_model.clone(),
            },
        };
        Ok(command.over(&project.over(&user)))
    }
}
#[derive(Debug, Clone, Default, Args)]
pub(super) struct ManagedSourceArguments {
    #[command(flatten)]
    pub(super) dependencies: ManagedArguments,
    #[arg(long)]
    ffmpeg_executable: Option<PathBuf>,
    #[arg(long)]
    ffprobe_executable: Option<PathBuf>,
    /// Requested spatial scale; managed preset defaults to four.
    #[arg(long)]
    output_scale: Option<u8>,
    /// Finite worst-case disk admission bound, not a reservation.
    #[arg(long)]
    maximum_working_bytes: Option<u64>,
    #[arg(long)]
    maximum_source_bytes: Option<u64>,
    #[arg(long)]
    maximum_video_output_bytes: Option<u64>,
    #[arg(long)]
    maximum_capture_bytes: Option<usize>,
    #[arg(long)]
    minimum_free_bytes: Option<u64>,
    /// Caller-selected PNG cap; exceeding it fails safely instead of truncating.
    #[arg(long)]
    maximum_output_frame_bytes: Option<u64>,
    #[arg(long)]
    maximum_input_frame_bytes: Option<u64>,
    #[arg(long)]
    maximum_frames: Option<u64>,
    #[arg(long)]
    maximum_total_frame_bytes: Option<u64>,
    #[arg(long)]
    frames_per_chunk: Option<u32>,
    #[arg(long)]
    chunk_timeout_milliseconds: Option<u64>,
    /// Input-global stream indices; defaults are video 0 and audio 1.
    #[arg(long)]
    video_stream: Option<u32>,
    #[arg(long, conflicts_with = "no_audio")]
    audio_stream: Option<u32>,
    #[arg(long)]
    no_audio: bool,
    #[arg(long, value_delimiter = ',')]
    discard_streams: Vec<u32>,
    /// Explicitly enable a fresh RIFE child; never enabled by dependency overrides.
    #[arg(long)]
    interpolate: bool,
    /// Operator-observed host RAM required when selecting a new managed RIFE child.
    #[arg(long)]
    rife_total_memory_mib: Option<u64>,
    #[arg(long)]
    rife_available_memory_mib: Option<u64>,
    /// -1 selects CPU; a nonnegative value selects one Vulkan device.
    #[arg(long, allow_hyphen_values = true)]
    rife_device_id: Option<i32>,
}
impl ManagedSourceArguments {
    pub(super) fn present(&self) -> bool {
        self.dependencies.present()
            || self.ffmpeg_executable.is_some()
            || self.ffprobe_executable.is_some()
            || self.output_scale.is_some()
            || self.maximum_working_bytes.is_some()
            || self.maximum_source_bytes.is_some()
            || self.maximum_video_output_bytes.is_some()
            || self.maximum_capture_bytes.is_some()
            || self.minimum_free_bytes.is_some()
            || self.maximum_output_frame_bytes.is_some()
            || self.maximum_input_frame_bytes.is_some()
            || self.maximum_frames.is_some()
            || self.maximum_total_frame_bytes.is_some()
            || self.frames_per_chunk.is_some()
            || self.chunk_timeout_milliseconds.is_some()
            || self.video_stream.is_some()
            || self.audio_stream.is_some()
            || self.no_audio
            || !self.discard_streams.is_empty()
            || self.interpolate
            || self.rife_total_memory_mib.is_some()
            || self.rife_available_memory_mib.is_some()
            || self.rife_device_id.is_some()
    }
    pub(super) fn resume_overrides_present(&self) -> bool {
        let mut selected = self.clone();
        selected.dependencies.offline = false;
        selected.dependencies.no_download = false;
        selected.present()
    }
    pub(super) fn apply(
        &self,
        mut configuration: ManagedMusicVideoConfiguration,
    ) -> Result<ManagedMusicVideoConfiguration> {
        let project_tools = configuration.tools.clone();
        configuration.tools = self.dependencies.effective(&project_tools)?;
        if let Some(path) = &self.ffmpeg_executable {
            configuration.ffmpeg = Some(path.clone());
        }
        if let Some(path) = &self.ffprobe_executable {
            configuration.ffprobe = Some(path.clone());
        }
        if let Some(scale) = self.output_scale {
            configuration.output_scale = scale;
        }
        if let Some(value) = self.maximum_source_bytes {
            configuration.maximum_source_bytes = value;
        }
        if let Some(value) = self.maximum_video_output_bytes {
            configuration.maximum_video_output_bytes = value;
        }
        if let Some(value) = self.maximum_capture_bytes {
            configuration.maximum_capture_bytes = value;
        }
        if let Some(value) = self.maximum_working_bytes {
            configuration.maximum_working_bytes = value;
        }
        if let Some(value) = self.minimum_free_bytes {
            configuration.minimum_free_bytes = value;
        }
        if let Some(value) = self.maximum_output_frame_bytes {
            configuration.maximum_output_frame_bytes = value;
        }
        if let Some(value) = self.maximum_input_frame_bytes {
            configuration.maximum_input_frame_bytes = value;
        }
        if let Some(value) = self.maximum_frames {
            configuration.maximum_frames = value;
        }
        if let Some(value) = self.maximum_total_frame_bytes {
            configuration.maximum_total_frame_bytes = value;
        }
        if let Some(value) = self.frames_per_chunk {
            configuration.frames_per_chunk = value;
        }
        if let Some(value) = self.chunk_timeout_milliseconds {
            configuration.chunk_timeout_milliseconds = value;
        }
        if let Some(stream) = self.video_stream {
            configuration.source_selection.video_stream = Some(stream);
        }
        if let Some(stream) = self.audio_stream {
            configuration.source_selection.audio_stream = Some(stream);
            configuration.source_selection.no_audio = false;
        }
        if self.no_audio {
            configuration.source_selection.audio_stream = None;
            configuration.source_selection.no_audio = true;
        }
        if !self.discard_streams.is_empty() {
            configuration.source_selection.discard_streams = self.discard_streams.clone();
        }
        if self.interpolate && configuration.interpolation.is_none() {
            let mut settings = self.rife(None)?;
            settings.tools = ManagedOptions::default();
            configuration.interpolation = Some(settings);
        }
        if let Some(settings) = configuration.interpolation.as_mut() {
            settings.tools = self
                .dependencies
                .effective(&settings.tools.over(&project_tools))?;
            if let Some(total) = self.rife_total_memory_mib {
                settings.host.total_memory_mib = total;
            }
            if let Some(available) = self.rife_available_memory_mib {
                settings.host.available_memory_mib = available;
            }
            if let Some(device) = self.rife_device_id {
                settings.device_id = device;
            }
        } else if self.rife_total_memory_mib.is_some()
            || self.rife_available_memory_mib.is_some()
            || self.rife_device_id.is_some()
        {
            return Err(Error::new(
                ErrorCategory::Configuration,
                "RIFE host/device options require --interpolate or selected interpolation in the managed configuration",
            ));
        }
        Ok(configuration)
    }
    pub(super) fn rife(
        &self,
        existing: Option<ManagedRifeConfiguration>,
    ) -> Result<ManagedRifeConfiguration> {
        let mut settings=match existing {Some(value)=>value,None=>ManagedRifeConfiguration {schema:managed::MANAGED_RIFE_SCHEMA.to_owned(),tools:ManagedOptions::default(),host:aniflow::video_interpolation::RifeHost {operating_system:std::env::consts::OS.to_owned(),architecture:std::env::consts::ARCH.to_owned(),total_memory_mib:self.rife_total_memory_mib.ok_or_else(||Error::new(ErrorCategory::Configuration,"selected managed RIFE needs --rife-total-memory-mib and --rife-available-memory-mib observations"))?,available_memory_mib:self.rife_available_memory_mib.ok_or_else(||Error::new(ErrorCategory::Configuration,"selected managed RIFE needs --rife-available-memory-mib"))?},device_id:-1,preset:aniflow::video_interpolation::RifePreset::Auto}};
        settings.tools = self.dependencies.effective(&settings.tools)?;
        if let Some(total) = self.rife_total_memory_mib {
            settings.host.total_memory_mib = total;
        }
        if let Some(available) = self.rife_available_memory_mib {
            settings.host.available_memory_mib = available;
        }
        if let Some(device) = self.rife_device_id {
            settings.device_id = device;
        }
        Ok(settings)
    }
}

pub(super) fn read_managed_source(bytes: &[u8]) -> Result<ManagedMusicVideoConfiguration> {
    if bytes.len() > 131_072 {
        return Err(Error::new(
            ErrorCategory::Configuration,
            "managed music-video configuration exceeds128KiB",
        ));
    }
    serde_json::from_slice(bytes).map_err(|error| {
        Error::new(
            ErrorCategory::Configuration,
            format!("invalid closed managed music-video configuration: {error}"),
        )
    })
}
pub(super) fn is_schema(bytes: &[u8], schema: &str) -> bool {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| {
            value
                .get("schema")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .as_deref()
        == Some(schema)
}

pub(super) fn prepare_source(
    source: &Path,
    configuration: &ManagedMusicVideoConfiguration,
    plan: bool,
    cancellation: &aniflow::CancellationToken,
    presentation: Presentation,
) -> Result<Option<aniflow::music_video_recipe::SourceMusicVideoConfiguration>> {
    configuration.validate()?;
    let catalog = bundled_catalog()?;
    let report = managed::plan_source_dependencies(&catalog, configuration)?;
    if plan && !report.ready {
        let preview = managed::plan_source_preview(source, configuration, &catalog)?;
        print_result(CommandName::VideoMusicVideo, presentation, &preview, || {
            println!(
                "Source: {} ({} bytes; file identity only)",
                preview.source_video.display(),
                preview.source_bytes
            );
            println!(
                "Requested spatial scale: {}x; geometry and native storage admission pending",
                configuration.output_scale
            );
            println!(
                "Bounds: {} working bytes, {} output PNG bytes/frame, {} frames/chunk",
                configuration.maximum_working_bytes,
                configuration.maximum_output_frame_bytes,
                configuration.frames_per_chunk
            );
            for effect in &preview.ordered_effects {
                println!("  {effect}");
            }
            println!("{}", report.next_action);
            for tool in &report.tools {
                println!(
                    "  {:?}: {} ({})",
                    tool.provider, tool.package_id, tool.backend
                );
                if let Some(acquisition) = &tool.acquisition {
                    println!(
                        "    {:?}; network{} bytes",
                        acquisition.state, acquisition.network_bytes
                    );
                }
                for note in &tool.notes {
                    println!("    {note}");
                }
            }
        })
        .map_err(|failure| failure.error)?;
        return Ok(None);
    }
    // Pin local mandatory tools before any managed network request.
    managed::local_ffmpeg_pin(configuration.ffmpeg.as_deref(), "ffmpeg")?;
    managed::local_ffmpeg_pin(configuration.ffprobe.as_deref(), "ffprobe")?;
    let control = managed_control(cancellation, presentation);
    let upscayl = managed::resolve_tool(
        &catalog,
        &configuration.tools,
        ManagedProvider::Upscayl,
        &HttpsDownloader,
        &control,
    )?;
    let rife = configuration
        .interpolation
        .as_ref()
        .map(|settings| {
            managed::resolve_tool(
                &catalog,
                &settings.tools,
                ManagedProvider::Rife,
                &HttpsDownloader,
                &control,
            )
        })
        .transpose()?;
    let native = managed::build_source(configuration, &upscayl, rife.as_ref())?;
    if !plan {
        let bytes = serde_json::to_vec(&native).map_err(|_| {
            Error::new(
                ErrorCategory::Internal,
                "cannot encode exact managed native configuration",
            )
        })?;
        managed::retain_native_configuration(&configuration.tools, &upscayl, &bytes)?;
        if let (Some(settings), Some(tool)) = (&configuration.interpolation, rife.as_ref()) {
            managed::retain_native_configuration(&settings.tools, tool, &bytes)?;
        }
    }
    Ok(Some(native))
}
fn print_managed_plan(
    report: &managed::ManagedExperiencePlan,
    presentation: Presentation,
    name: CommandName,
) -> CommandResult<()> {
    print_result(name, presentation, report, || {
        println!("Managed dependencies ready: {}", report.ready);
        println!("Cache: {}", report.cache_directory.display());
        for tool in &report.tools {
            println!(
                "  {:?}: {} / {} ({})",
                tool.provider, tool.package_id, tool.model, tool.backend
            );
            if let Some(acquisition) = &tool.acquisition {
                println!(
                    "    {:?}; network{} bytes; installed{} bytes",
                    acquisition.state, acquisition.network_bytes, acquisition.installed_bytes
                );
            }
            for note in &tool.notes {
                println!("    {note}");
            }
        }
        println!("{}", report.next_action);
        println!("Native qualification: {}", report.native_qualification);
    })
}

pub(super) fn retained_source_configuration(directory: &Path) -> Result<Vec<u8>> {
    let bytes = read_bounded_configuration(
        &directory.join("intent.json"),
        "retained source recipe intent",
        262_144,
    )?;
    let intent: aniflow::music_video_recipe::SourceMusicVideoIntent =
        serde_json::from_slice(&bytes).map_err(|_| {
            Error::new(
                ErrorCategory::State,
                "resume without --configuration requires a valid retained source-entry intent",
            )
        })?;
    intent.validate()?;
    serde_json::to_vec(&intent.payload.configuration).map_err(|_| {
        Error::new(
            ErrorCategory::Internal,
            "cannot encode retained source configuration",
        )
    })
}

#[derive(Debug, Clone, Default, Args)]
pub(super) struct ManagedRifeArguments {
    #[command(flatten)]
    dependencies: ManagedArguments,
    #[arg(long)]
    rife_total_memory_mib: Option<u64>,
    #[arg(long)]
    rife_available_memory_mib: Option<u64>,
    #[arg(long, allow_hyphen_values = true)]
    rife_device_id: Option<i32>,
}
impl ManagedRifeArguments {
    pub(super) fn present(&self) -> bool {
        self.dependencies.overrides_present()
            || self.rife_total_memory_mib.is_some()
            || self.rife_available_memory_mib.is_some()
            || self.rife_device_id.is_some()
    }
    fn settings(
        &self,
        configuration: Option<ManagedRifeConfiguration>,
    ) -> Result<ManagedRifeConfiguration> {
        ManagedSourceArguments {
            dependencies: self.dependencies.clone(),
            rife_total_memory_mib: self.rife_total_memory_mib,
            rife_available_memory_mib: self.rife_available_memory_mib,
            rife_device_id: self.rife_device_id,
            ..ManagedSourceArguments::default()
        }
        .rife(configuration)
    }
    pub(super) fn prepare(
        &self,
        bytes: Option<&[u8]>,
        plan: bool,
        cancellation: &aniflow::CancellationToken,
        presentation: Presentation,
        name: CommandName,
        paths: &[PathBuf],
    ) -> Result<Option<aniflow::video_interpolation::RifeConfiguration>> {
        let configuration = if let Some(bytes) = bytes {
            if !is_schema(bytes, managed::MANAGED_RIFE_SCHEMA) {
                if self.present() {
                    return Err(Error::new(
                        ErrorCategory::Configuration,
                        "managed RIFE flags require omitted configuration or the managed RIFE schema; explicit legacy pins are unchanged",
                    ));
                }
                return aniflow::video_interpolation::RifeConfiguration::from_json_slice(bytes)
                    .map(Some);
            }
            Some(
                serde_json::from_slice::<ManagedRifeConfiguration>(bytes).map_err(|_| {
                    Error::new(
                        ErrorCategory::Configuration,
                        "invalid closed managed RIFE configuration",
                    )
                })?,
            )
        } else {
            None
        };
        let project = configuration
            .as_ref()
            .map(|value| &value.tools)
            .cloned()
            .unwrap_or_default();
        let effective = self.dependencies.effective(&project)?;
        managed::guard_cache_separation(&effective, paths)?;
        let catalog = bundled_catalog()?;
        let report = managed::plan_experience(&catalog, &effective, false, true)?;
        if plan && !report.ready {
            print_managed_plan(&report, presentation, name).map_err(|failure| failure.error)?;
            return Ok(None);
        }
        let settings = self.settings(configuration)?;
        settings.validate()?;
        let tool = managed::resolve_tool(
            &catalog,
            &effective,
            ManagedProvider::Rife,
            &HttpsDownloader,
            &managed_control(cancellation, presentation),
        )?;
        let native = managed::build_rife(&settings, &tool)?;
        if !plan {
            let bytes = serde_json::to_vec(&native).map_err(|_| {
                Error::new(
                    ErrorCategory::Internal,
                    "cannot encode exact managed RIFE configuration",
                )
            })?;
            managed::retain_native_configuration(&effective, &tool, &bytes)?;
        }
        Ok(Some(native))
    }
}

fn managed_control(
    cancellation: &aniflow::CancellationToken,
    presentation: Presentation,
) -> ManagedControl {
    use aniflow::toolchain::managed::ManagedProgress;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let observer = if presentation == Presentation::Human {
        let last = Arc::new(AtomicU64::new(0));
        Some(Arc::new(move |progress: ManagedProgress| match progress {
            ManagedProgress::DownloadStarted { artifact_id, bytes } => {
                last.store(0, Ordering::Relaxed);
                eprintln!("Acquiring managed {artifact_id} ({bytes} bytes)…");
            }
            ManagedProgress::DownloadProgress {
                artifact_id,
                bytes,
                total,
            } => {
                let bucket = bytes / 16_777_216;
                if bucket > last.load(Ordering::Relaxed) {
                    last.store(bucket, Ordering::Relaxed);
                    eprintln!("  {artifact_id}: {bytes}/{total} bytes");
                }
            }
            ManagedProgress::ArchiveVerified { artifact_id } => {
                eprintln!("  Verified {artifact_id}; preparing isolated installation…")
            }
            ManagedProgress::Installed { package_id } => {
                eprintln!("Managed package ready: {package_id}")
            }
            ManagedProgress::FileInstalled { .. } => {}
        }) as Arc<dyn Fn(ManagedProgress) + Send + Sync>)
    } else {
        None
    };
    ManagedControl {
        cancellation: cancellation.clone(),
        observer,
        ..ManagedControl::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_source_defaults_parse_without_full_pin_configuration() {
        let parsed = Cli::try_parse_from([
            "aniflow",
            "video",
            "music-video",
            "--source-video",
            "clip.mp4",
            "--plan",
            "--offline",
            "--maximum-source-bytes",
            "1048576",
            "--maximum-video-output-bytes",
            "2097152",
        ])
        .unwrap();
        let Commands::Video {
            command:
                VideoCommands::MusicVideo {
                    configuration,
                    managed,
                    ..
                },
        } = parsed.command
        else {
            panic!("music-video")
        };
        assert!(configuration.is_none());
        let effective = managed
            .apply(ManagedMusicVideoConfiguration::default())
            .unwrap();
        assert!(effective.tools.offline);
        assert_eq!(effective.maximum_source_bytes, 1_048_576);
        assert_eq!(effective.maximum_video_output_bytes, 2_097_152);
        assert!(effective.interpolation.is_none());
    }
    #[test]
    fn project_nested_rife_beats_global_and_user_while_cli_beats_project() {
        let temporary = tempfile::tempdir().unwrap();
        let user = temporary.path().join("user.json");
        std::fs::write(
            &user,
            serde_json::to_vec(&managed::ManagedUserConfiguration {
                schema: managed::MANAGED_OPTIONS_SCHEMA.to_owned(),
                options: ManagedOptions {
                    rife: ManagedToolOptions {
                        version: Some("1.0.0".to_owned()),
                        ..ManagedToolOptions::default()
                    },
                    ..ManagedOptions::default()
                },
            })
            .unwrap(),
        )
        .unwrap();
        let mut project = ManagedMusicVideoConfiguration::default();
        project.tools.rife.version = Some("2.0.0".to_owned());
        project.interpolation = Some(ManagedRifeConfiguration {
            schema: managed::MANAGED_RIFE_SCHEMA.to_owned(),
            tools: ManagedOptions {
                rife: ManagedToolOptions {
                    version: Some("3.0.0".to_owned()),
                    ..ManagedToolOptions::default()
                },
                ..ManagedOptions::default()
            },
            host: aniflow::video_interpolation::RifeHost {
                operating_system: std::env::consts::OS.to_owned(),
                architecture: std::env::consts::ARCH.to_owned(),
                total_memory_mib: 8192,
                available_memory_mib: 4096,
            },
            device_id: -1,
            preset: aniflow::video_interpolation::RifePreset::Auto,
        });
        let mut args = ManagedSourceArguments::default();
        args.dependencies.managed_user_configuration = Some(user);
        assert_eq!(
            args.apply(project.clone())
                .unwrap()
                .interpolation
                .unwrap()
                .tools
                .rife
                .version
                .as_deref(),
            Some("3.0.0")
        );
        args.dependencies.rife_version = Some("4.0.0".to_owned());
        assert_eq!(
            args.apply(project)
                .unwrap()
                .interpolation
                .unwrap()
                .tools
                .rife
                .version
                .as_deref(),
            Some("4.0.0")
        );
    }
    #[test]
    fn offline_policy_is_allowed_with_retained_configuration_without_override() {
        let mut args = ManagedSourceArguments::default();
        args.dependencies.offline = true;
        args.dependencies.no_download = true;
        assert!(!args.resume_overrides_present());
        args.dependencies.upscayl_model = Some("anime".to_owned());
        assert!(args.resume_overrides_present());
    }
}
