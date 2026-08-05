use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct RunWorkspace {
    pub root: PathBuf,
}

impl RunWorkspace {
    pub fn create(parent: Option<&Path>, pipeline_name: &str) -> Result<Self> {
        let parent = parent
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".aniflow/runs"));
        let safe_name = sanitize_name(pipeline_name);
        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
        let root = parent.join(format!("{timestamp}-{safe_name}"));
        let workspace = Self { root };
        workspace.create_directories()?;
        Ok(workspace)
    }

    pub fn open(root: &Path) -> Result<Self> {
        let workspace = Self {
            root: root.to_path_buf(),
        };
        if !workspace.manifest().is_file() {
            anyhow::bail!(
                "{} is not an aniflow run directory",
                workspace.root.display()
            );
        }
        workspace.create_directories()?;
        Ok(workspace)
    }

    pub fn create_directories(&self) -> Result<()> {
        for directory in [
            self.config(),
            self.logs(),
            self.metadata(),
            self.source_frames(),
            self.frame_stages(),
            self.audio(),
            self.audio_stages(),
            self.subtitles(),
            self.video(),
            self.video_stages(),
            self.output(),
            self.renderflow(),
            self.delivery(),
            self.state(),
        ] {
            fs::create_dir_all(&directory)
                .with_context(|| format!("failed to create {}", directory.display()))?;
        }
        Ok(())
    }

    pub fn manifest(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    pub fn pipeline_copy(&self) -> PathBuf {
        self.config().join("pipeline.yml")
    }

    pub fn config(&self) -> PathBuf {
        self.root.join("config")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn metadata(&self) -> PathBuf {
        self.root.join("metadata")
    }

    pub fn source_frames(&self) -> PathBuf {
        self.root.join("frames/source")
    }

    pub fn frame_stages(&self) -> PathBuf {
        self.root.join("frames/stages")
    }

    pub fn frame_stage(&self, index: usize, id: &str) -> PathBuf {
        self.frame_stages().join(format!("{:02}-{id}", index + 1))
    }

    pub fn audio(&self) -> PathBuf {
        self.root.join("audio")
    }

    pub fn audio_stages(&self) -> PathBuf {
        self.audio().join("stages")
    }

    pub fn audio_stage_file(&self, index: usize, id: &str, extension: &str) -> PathBuf {
        self.audio_stages()
            .join(format!("{:02}-{id}.{extension}", index + 1))
    }

    pub fn subtitles(&self) -> PathBuf {
        self.root.join("subtitles")
    }

    pub fn video(&self) -> PathBuf {
        self.root.join("video")
    }

    pub fn video_stages(&self) -> PathBuf {
        self.video().join("stages")
    }

    pub fn video_stage_file(&self, index: usize, id: &str, extension: &str) -> PathBuf {
        self.video_stages()
            .join(format!("{:02}-{id}.{extension}", index + 1))
    }

    pub fn output(&self) -> PathBuf {
        self.root.join("output")
    }

    pub fn renderflow(&self) -> PathBuf {
        self.root.join("renderflow")
    }

    pub fn delivery(&self) -> PathBuf {
        self.root.join("delivery")
    }

    pub fn delivery_manifest(&self) -> PathBuf {
        self.delivery().join("manifest.json")
    }

    pub fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn stage_marker(&self, stage: &str) -> PathBuf {
        self.state().join(format!("{stage}.complete"))
    }

    pub fn stage_log(&self, stage: &str) -> PathBuf {
        self.logs().join(format!("{stage}.log"))
    }
}

fn sanitize_name(value: &str) -> String {
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
    let sanitized = sanitized.trim_matches('-').to_lowercase();
    if sanitized.is_empty() {
        "pipeline".to_owned()
    } else {
        sanitized
    }
}
