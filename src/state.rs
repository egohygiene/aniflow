use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::facade::{
    ArtifactStatus as PublicArtifactStatus, ProgressState, RunStatus,
    StageStatus as PublicStageStatus,
};
use crate::media::MediaInspection;
use crate::workspace::RunWorkspace;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema_version: u32,
    pub aniflow_version: String,
    pub run_id: String,
    pub pipeline_name: String,
    pub pipeline_file: PathBuf,
    pub source_file: PathBuf,
    pub source_sha256: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub inspection: MediaInspection,
    pub stages: BTreeMap<String, StageRecord>,
    pub artifacts: BTreeMap<String, ArtifactRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageRecord {
    pub status: StageStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub path: PathBuf,
    pub sha256: Option<String>,
}

impl RunManifest {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("invalid manifest {}", path.display()))
    }

    pub fn save(&mut self, path: &Path) -> Result<()> {
        self.updated_at = Utc::now();
        let contents = serde_json::to_string_pretty(self)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, contents)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        fs::rename(&temporary, path)
            .with_context(|| format!("failed to publish {}", path.display()))
    }

    pub fn stage_running(&mut self, stage: &str) {
        let record = self.stage_mut(stage);
        record.status = StageStatus::Running;
        record.started_at = Some(Utc::now());
        record.completed_at = None;
        record.message = None;
    }

    pub fn stage_complete(&mut self, stage: &str, message: Option<String>) {
        let record = self.stage_mut(stage);
        record.status = StageStatus::Complete;
        record.completed_at = Some(Utc::now());
        record.message = message;
    }

    pub fn stage_failed(&mut self, stage: &str, message: String) {
        let record = self.stage_mut(stage);
        record.status = StageStatus::Failed;
        record.completed_at = Some(Utc::now());
        record.message = Some(message);
    }

    fn stage_mut(&mut self, stage: &str) -> &mut StageRecord {
        self.stages
            .entry(stage.to_owned())
            .or_insert_with(|| StageRecord {
                status: StageStatus::Pending,
                started_at: None,
                completed_at: None,
                message: None,
            })
    }
}

pub fn status(run_directory: &Path) -> Result<RunStatus> {
    let workspace = RunWorkspace::open(run_directory)?;
    let manifest = RunManifest::load(&workspace.manifest())?;

    Ok(RunStatus {
        run_id: manifest.run_id,
        pipeline_name: manifest.pipeline_name,
        source_file: manifest.source_file,
        stages: manifest
            .stages
            .into_iter()
            .map(|(name, record)| PublicStageStatus {
                name,
                state: match record.status {
                    StageStatus::Pending => ProgressState::Waiting,
                    StageStatus::Running => ProgressState::Running,
                    StageStatus::Complete => ProgressState::Complete,
                    StageStatus::Failed => ProgressState::Failed,
                },
                message: record.message,
            })
            .collect(),
        artifacts: manifest
            .artifacts
            .into_iter()
            .map(|(name, artifact)| PublicArtifactStatus {
                name,
                path: artifact.path,
                sha256: artifact.sha256,
            })
            .collect(),
    })
}
