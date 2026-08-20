use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pipeline {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub frame_processors: Vec<FrameProcessor>,
    #[serde(default)]
    pub validation: FrameValidation,
    #[serde(default)]
    pub audio_processors: Vec<CommandProcessor>,
    #[serde(default)]
    pub subtitles: Option<Subtitles>,
    #[serde(default)]
    pub video_processors: Vec<CommandProcessor>,
    #[serde(default)]
    pub renderflow: Option<RenderflowHandoff>,
    #[serde(default)]
    pub output: Output,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FrameProcessor {
    External {
        id: String,
        #[serde(default = "enabled")]
        enabled: bool,
        command: String,
        #[serde(default)]
        arguments: Vec<String>,
        #[serde(default = "default_concurrency")]
        concurrency: usize,
    },
    UpscaylNcnn {
        id: String,
        #[serde(default = "enabled")]
        enabled: bool,
        #[serde(default = "default_upscayl_command")]
        command: String,
        #[serde(default = "default_upscayl_model")]
        model: String,
        #[serde(default)]
        model_path: Option<PathBuf>,
        #[serde(default = "default_upscale")]
        scale: u8,
        #[serde(default)]
        tile_size: Option<u32>,
        #[serde(default)]
        gpu_id: Option<String>,
        #[serde(default)]
        tta: bool,
        #[serde(default)]
        additional_arguments: Vec<String>,
    },
    GeminiWatermarkRemover {
        id: String,
        #[serde(default = "enabled")]
        enabled: bool,
        #[serde(default = "default_gwr_command")]
        command: String,
        #[serde(default = "enabled")]
        json: bool,
        #[serde(default)]
        additional_arguments: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandProcessor {
    pub id: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub command: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub output_extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameValidation {
    #[serde(default = "enabled")]
    pub require_uniform_dimensions: bool,
    #[serde(default = "default_minimum_frame_bytes")]
    pub minimum_frame_bytes: u64,
}

impl Default for FrameValidation {
    fn default() -> Self {
        Self {
            require_uniform_dimensions: true,
            minimum_frame_bytes: default_minimum_frame_bytes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subtitles {
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub source: PathBuf,
    #[serde(default)]
    pub mode: SubtitleMode,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleMode {
    #[default]
    Burn,
    Mux,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderflowHandoff {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_renderflow_command")]
    pub command: String,
    #[serde(default = "default_renderflow_arguments")]
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    #[serde(default = "default_output_file")]
    pub file: PathBuf,
    #[serde(default = "default_video_codec")]
    pub video_codec: String,
    #[serde(default = "default_crf")]
    pub crf: u8,
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default = "default_pixel_format")]
    pub pixel_format: String,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            file: default_output_file(),
            video_codec: default_video_codec(),
            crf: default_crf(),
            preset: default_preset(),
            pixel_format: default_pixel_format(),
        }
    }
}

impl Pipeline {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read pipeline {}", path.display()))?;
        let pipeline: Self = serde_yaml::from_str(&contents)
            .with_context(|| format!("invalid pipeline YAML in {}", path.display()))?;
        pipeline.validate()?;
        Ok(pipeline)
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 2 {
            bail!("unsupported pipeline version {}; expected 2", self.version);
        }
        if self.name.trim().is_empty() {
            bail!("pipeline name cannot be empty");
        }
        validate_safe_output_path(&self.output.file, "output.file", "output")?;
        if !(0..=51).contains(&self.output.crf) {
            bail!("output.crf must be between 0 and 51");
        }
        if self.validation.minimum_frame_bytes == 0 {
            bail!("validation.minimum_frame_bytes must be at least 1");
        }

        validate_unique_ids(
            self.frame_processors.iter().map(FrameProcessor::id),
            "frame_processors",
        )?;
        for processor in &self.frame_processors {
            processor.validate()?;
        }

        validate_command_processors(&self.audio_processors, "audio_processors")?;
        validate_command_processors(&self.video_processors, "video_processors")?;

        if let Some(renderflow) = self.renderflow.as_ref().filter(|value| value.enabled) {
            validate_command(&renderflow.command, "renderflow.command")?;
            validate_input_output_arguments(&renderflow.arguments, "renderflow.arguments")?;
        }

        Ok(())
    }

    pub fn stage_names(&self, has_audio: bool) -> Vec<String> {
        let mut stages = vec!["inspect".to_owned(), "extract".to_owned()];
        for (index, processor) in self.enabled_frame_processors().enumerate() {
            stages.push(format!(
                "frame_{:02}_{}",
                index + 1,
                sanitize_identifier(processor.id())
            ));
        }
        stages.push("validate_frames".to_owned());
        stages.push("assemble_video".to_owned());
        if has_audio {
            for (index, processor) in self.enabled_audio_processors().enumerate() {
                stages.push(format!(
                    "audio_{:02}_{}",
                    index + 1,
                    sanitize_identifier(&processor.id)
                ));
            }
            stages.push("restore_audio".to_owned());
        }
        if self.subtitles.as_ref().is_some_and(|value| value.enabled) {
            stages.push("subtitles".to_owned());
        }
        for (index, processor) in self.enabled_video_processors().enumerate() {
            stages.push(format!(
                "video_{:02}_{}",
                index + 1,
                sanitize_identifier(&processor.id)
            ));
        }
        stages.push("finalize".to_owned());
        if self.renderflow.as_ref().is_some_and(|value| value.enabled) {
            stages.push("renderflow".to_owned());
        }
        stages.push("delivery".to_owned());
        stages
    }

    pub fn enabled_frame_processors(&self) -> impl Iterator<Item = &FrameProcessor> {
        self.frame_processors
            .iter()
            .filter(|processor| processor.enabled())
    }

    pub fn enabled_audio_processors(&self) -> impl Iterator<Item = &CommandProcessor> {
        self.audio_processors
            .iter()
            .filter(|processor| processor.enabled)
    }

    pub fn enabled_video_processors(&self) -> impl Iterator<Item = &CommandProcessor> {
        self.video_processors
            .iter()
            .filter(|processor| processor.enabled)
    }

    pub fn required_commands(&self) -> Vec<String> {
        let mut commands = BTreeSet::from(["ffmpeg".to_owned(), "ffprobe".to_owned()]);
        commands.extend(
            self.enabled_frame_processors()
                .map(|processor| processor.command().to_owned()),
        );
        commands.extend(
            self.enabled_audio_processors()
                .map(|processor| processor.command.clone()),
        );
        commands.extend(
            self.enabled_video_processors()
                .map(|processor| processor.command.clone()),
        );
        if let Some(renderflow) = self.renderflow.as_ref().filter(|value| value.enabled) {
            commands.insert(renderflow.command.clone());
        }
        commands.into_iter().collect()
    }
}

impl FrameProcessor {
    pub fn id(&self) -> &str {
        match self {
            Self::External { id, .. }
            | Self::UpscaylNcnn { id, .. }
            | Self::GeminiWatermarkRemover { id, .. } => id,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            Self::External { enabled, .. }
            | Self::UpscaylNcnn { enabled, .. }
            | Self::GeminiWatermarkRemover { enabled, .. } => *enabled,
        }
    }

    pub fn command(&self) -> &str {
        match self {
            Self::External { command, .. }
            | Self::UpscaylNcnn { command, .. }
            | Self::GeminiWatermarkRemover { command, .. } => command,
        }
    }

    pub fn concurrency(&self) -> Option<usize> {
        match self {
            Self::External { concurrency, .. } => Some(*concurrency),
            Self::UpscaylNcnn { .. } | Self::GeminiWatermarkRemover { .. } => None,
        }
    }

    pub fn is_batch(&self) -> bool {
        !matches!(self, Self::External { .. })
    }

    pub fn arguments(&self, input: &Path, output: &Path) -> Vec<String> {
        match self {
            Self::External { arguments, .. } => arguments.clone(),
            Self::UpscaylNcnn {
                model,
                model_path,
                scale,
                tile_size,
                gpu_id,
                tta,
                additional_arguments,
                ..
            } => {
                let mut arguments = vec![
                    "-i".to_owned(),
                    input.to_string_lossy().into_owned(),
                    "-o".to_owned(),
                    output.to_string_lossy().into_owned(),
                    "-n".to_owned(),
                    model.clone(),
                    "-s".to_owned(),
                    scale.to_string(),
                    "-f".to_owned(),
                    "png".to_owned(),
                ];
                if let Some(model_path) = model_path {
                    arguments.extend(["-m".to_owned(), model_path.to_string_lossy().into_owned()]);
                }
                if let Some(tile_size) = tile_size {
                    arguments.extend(["-t".to_owned(), tile_size.to_string()]);
                }
                if let Some(gpu_id) = gpu_id {
                    arguments.extend(["-g".to_owned(), gpu_id.clone()]);
                }
                if *tta {
                    arguments.push("-x".to_owned());
                }
                arguments.extend(additional_arguments.iter().cloned());
                arguments
            }
            Self::GeminiWatermarkRemover {
                json,
                additional_arguments,
                ..
            } => {
                let mut arguments = vec![
                    "remove".to_owned(),
                    input.to_string_lossy().into_owned(),
                    "--out-dir".to_owned(),
                    output.to_string_lossy().into_owned(),
                    "--overwrite".to_owned(),
                ];
                if *json {
                    arguments.push("--json".to_owned());
                }
                arguments.extend(additional_arguments.iter().cloned());
                arguments
            }
        }
    }

    fn validate(&self) -> Result<()> {
        validate_identifier(self.id(), "frame processor id")?;
        validate_command(self.command(), "frame processor command")?;
        if self
            .concurrency()
            .is_some_and(|concurrency| concurrency == 0)
        {
            bail!(
                "frame processor `{}` concurrency must be at least 1",
                self.id()
            );
        }

        match self {
            Self::External { arguments, .. } => {
                validate_input_output_arguments(arguments, "external frame processor arguments")?;
            }
            Self::UpscaylNcnn {
                scale,
                model,
                model_path,
                ..
            } => {
                if !matches!(*scale, 2..=4) {
                    bail!("Upscayl processor `{}` scale must be 2, 3, or 4", self.id());
                }
                if model.trim().is_empty() {
                    bail!("Upscayl processor `{}` model cannot be empty", self.id());
                }
                if model_path.as_ref().is_some_and(|path| !path.is_absolute()) {
                    bail!(
                        "Upscayl processor `{}` model_path must be absolute",
                        self.id()
                    );
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn expand_argument(
    template: &str,
    input: &Path,
    output: &Path,
    frame_name: &str,
    run_directory: &Path,
) -> String {
    template
        .replace("{input}", &input.to_string_lossy())
        .replace("{output}", &output.to_string_lossy())
        .replace("{frame}", frame_name)
        .replace("{run_dir}", &run_directory.to_string_lossy())
}

pub fn sanitize_identifier(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    sanitized.trim_matches('-').to_lowercase()
}

fn validate_command_processors(processors: &[CommandProcessor], field: &str) -> Result<()> {
    validate_unique_ids(
        processors.iter().map(|processor| processor.id.as_str()),
        field,
    )?;
    for processor in processors {
        validate_identifier(&processor.id, "command processor id")?;
        if processor.enabled {
            validate_command(&processor.command, "command processor command")?;
            validate_input_output_arguments(&processor.arguments, "command processor arguments")?;
            if processor
                .output_extension
                .as_ref()
                .is_some_and(|extension| {
                    extension.is_empty()
                        || !extension
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric())
                })
            {
                bail!(
                    "processor `{}` output_extension must be alphanumeric without a dot",
                    processor.id
                );
            }
        }
    }
    Ok(())
}

fn validate_unique_ids<'a>(ids: impl Iterator<Item = &'a str>, field: &str) -> Result<()> {
    let mut known = BTreeSet::new();
    for id in ids {
        validate_identifier(id, field)?;
        if !known.insert(id) {
            bail!("{field} contains duplicate id `{id}`");
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{field} cannot be empty");
    }
    if sanitize_identifier(value) != value {
        bail!("{field} `{value}` must use lowercase letters, numbers, hyphens, or underscores");
    }
    Ok(())
}

fn validate_command(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{field} cannot be empty");
    }
    Ok(())
}

fn validate_input_output_arguments(arguments: &[String], field: &str) -> Result<()> {
    if !arguments
        .iter()
        .any(|argument| argument.contains("{input}"))
    {
        bail!("{field} must contain {{input}}");
    }
    if !arguments
        .iter()
        .any(|argument| argument.contains("{output}"))
    {
        bail!("{field} must contain {{output}}");
    }
    Ok(())
}

fn validate_safe_output_path(path: &Path, field: &str, root: &str) -> Result<()> {
    if path.is_absolute() {
        bail!("{field} must be relative to the run directory");
    }
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("{field} cannot escape the run directory");
    }
    if !matches!(
        path.components().next(),
        Some(std::path::Component::Normal(component))
            if component == std::ffi::OsStr::new(root)
    ) {
        bail!("{field} must be located beneath the run {root}/ directory");
    }
    Ok(())
}

const fn enabled() -> bool {
    true
}

const fn default_concurrency() -> usize {
    1
}

const fn default_minimum_frame_bytes() -> u64 {
    64
}

fn default_upscayl_command() -> String {
    "upscayl-bin".to_owned()
}

fn default_upscayl_model() -> String {
    "realesr-animevideov3".to_owned()
}

const fn default_upscale() -> u8 {
    2
}

fn default_gwr_command() -> String {
    "gwr".to_owned()
}

fn default_renderflow_command() -> String {
    "renderflow".to_owned()
}

fn default_renderflow_arguments() -> Vec<String> {
    vec![
        "run".to_owned(),
        "--input".to_owned(),
        "{input}".to_owned(),
        "--output-directory".to_owned(),
        "{output}".to_owned(),
    ]
}

fn default_output_file() -> PathBuf {
    PathBuf::from("output/master.mp4")
}

fn default_video_codec() -> String {
    "libx264".to_owned()
}

const fn default_crf() -> u8 {
    18
}

fn default_preset() -> String {
    "slow".to_owned()
}

fn default_pixel_format() -> String {
    "yuv420p".to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{FrameProcessor, Pipeline, expand_argument};

    #[test]
    fn expands_processor_placeholders() {
        let actual = expand_argument(
            "--input={input};--output={output};--name={frame};--run={run_dir}",
            Path::new("/tmp/source frame.png"),
            Path::new("/tmp/output frame.png"),
            "frame-00000001.png",
            Path::new("/tmp/run"),
        );

        assert_eq!(
            actual,
            "--input=/tmp/source frame.png;--output=/tmp/output frame.png;\
             --name=frame-00000001.png;--run=/tmp/run"
        );
    }

    #[test]
    fn builds_gemini_watermark_remover_arguments() {
        let processor = FrameProcessor::GeminiWatermarkRemover {
            id: "remove-watermark".to_owned(),
            enabled: true,
            command: "gwr".to_owned(),
            json: true,
            additional_arguments: Vec::new(),
        };

        assert_eq!(
            processor.arguments(Path::new("input-frames"), Path::new("output-frames")),
            vec![
                "remove",
                "input-frames",
                "--out-dir",
                "output-frames",
                "--overwrite",
                "--json",
            ]
        );
    }

    #[test]
    fn builds_upscayl_arguments() {
        let processor = FrameProcessor::UpscaylNcnn {
            id: "upscale".to_owned(),
            enabled: true,
            command: "upscayl-bin".to_owned(),
            model: "realesr-animevideov3".to_owned(),
            model_path: Some(PathBuf::from("/models")),
            scale: 2,
            tile_size: Some(256),
            gpu_id: Some("0".to_owned()),
            tta: false,
            additional_arguments: Vec::new(),
        };

        assert_eq!(
            processor.arguments(Path::new("input.png"), Path::new("output.png")),
            vec![
                "-i",
                "input.png",
                "-o",
                "output.png",
                "-n",
                "realesr-animevideov3",
                "-s",
                "2",
                "-f",
                "png",
                "-m",
                "/models",
                "-t",
                "256",
                "-g",
                "0",
            ]
        );
    }

    #[test]
    fn parses_shipped_pipeline_packs() {
        for source in [
            include_str!("../pipelines/passthrough.yml"),
            include_str!("../pipelines/anime-upscale.example.yml"),
            include_str!("../pipelines/gemini-clean-upscale.example.yml"),
            include_str!("../pipelines/lyrics.example.yml"),
        ] {
            let pipeline: Pipeline =
                serde_yaml::from_str(source).expect("shipped pipeline should deserialize");
            pipeline
                .validate()
                .expect("shipped pipeline should validate");
        }
    }

    #[test]
    fn reports_first_class_processor_dependencies() {
        let pipeline: Pipeline = serde_yaml::from_str(include_str!(
            "../pipelines/gemini-clean-upscale.example.yml"
        ))
        .expect("pipeline should deserialize");

        assert_eq!(
            pipeline.required_commands(),
            vec!["ffmpeg", "ffprobe", "gwr", "upscayl-bin"]
        );
    }
}
