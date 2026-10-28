use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use crate::audio_inspection::process::open_regular;
use crate::frame_io::identity;
use crate::provider::{canonical_json_bytes, decode_json};
use sha2::{Digest, Sha256};
use super::*;

pub(super) fn require_existing_canonical(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|value| matches!(value,Component::CurDir|Component::ParentDir))
        || path.canonicalize().ok().as_deref() != Some(path) {
        return Err(state("recipe paths and existing ancestors must be canonical without symbolic links or traversal"));
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| state("cannot inspect recipe path"))?;
    if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) { return Err(state("recipe paths must be regular files or directories")); }
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() { Ok(path.to_path_buf()) } else { Ok(std::env::current_dir().map_err(|_| state("cannot resolve recipe path"))?.join(path)) }
}

pub(super) fn root(path: &Path) -> Result<PathBuf> {
    let path = absolute(path)?;
    require_existing_canonical(&path)?;
    if !path.is_dir() { return Err(state("recipe run directory must be a directory")); }
    Ok(path)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    require_existing_canonical(path)?;
    let observed = identity(path,MAXIMUM_DOCUMENT_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(path).map_err(|_| state("cannot safely open recipe authority"))?.take(MAXIMUM_DOCUMENT_BYTES+1).read_to_end(&mut bytes)
        .map_err(|_| state("cannot read bounded recipe authority"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64) != observed || identity(path,MAXIMUM_DOCUMENT_BYTES)? != observed { return Err(state("recipe authority changed during observation")); }
    Ok(bytes)
}

pub(super) fn read_document<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    decode_json(&read_bytes(path)?,"recipe authority")
}

pub(super) fn read_intent(root: &Path) -> Result<MusicVideoIntent> {
    let intent: MusicVideoIntent = read_document(&root.join("intent.json"))?;
    intent.validate()?;
    Ok(intent)
}

pub(super) fn child(root: &Path, phase: MusicVideoPhase) -> PathBuf { root.join("children").join(phase.id()) }

pub(super) fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            require_existing_canonical(path)?;
            if !metadata.is_dir() { return Err(state("recipe child slot must be a v3 directory")); }
            Ok(true)
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(state("cannot inspect recipe child slot")),
    }
}

pub(super) fn validate_layout(root: &Path, intent: &MusicVideoIntent) -> Result<()> {
    require_existing_canonical(root)?;
    let children = root.join("children");
    require_existing_canonical(&children)?;
    if !children.is_dir() { return Err(state("recipe children directory is missing")); }
    for entry in fs::read_dir(root).map_err(|_| state("cannot enumerate recipe workspace"))? {
        let entry = entry.map_err(|_| state("cannot inspect recipe workspace entry"))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| state("recipe workspace entry must have a UTF-8 name"))?;
        require_existing_canonical(&entry.path())?;
        let admitted = match name {
            "intent.json" | "writer.lock" => entry.path().is_file(),
            "children" => entry.path().is_dir(),
            // Retained source admission failures are observational scratch,
            // never child-run authority or a source of replacement artifacts.
            _ if name.starts_with(".assembly-source-") => entry.path().is_dir(),
            _ => false,
        };
        if !admitted { return Err(state("recipe workspace contains an unexpected entry; preserve it for explicit reconciliation")); }
    }
    for entry in fs::read_dir(&children).map_err(|_| state("cannot enumerate recipe children"))? {
        let entry = entry.map_err(|_| state("cannot inspect recipe child"))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| state("recipe child name is not UTF-8"))?;
        if !matches!(name,"prepare_frames"|"upscale"|"assembly")
            || (name == "upscale" && matches!(intent.payload.configuration.upscale,MusicVideoUpscale::Disabled {..})) {
            return Err(state("recipe has an unknown or disabled child slot; loose runs cannot be adopted"));
        }
        if !exists(&entry.path())? { return Err(state("recipe child disappeared while observing layout")); }
    }
    Ok(())
}

pub(super) fn create(request: &MusicVideoRequest, parent: Option<PathBuf>, intent: &MusicVideoIntent) -> Result<PathBuf> {
    let parent = absolute(parent.as_deref().unwrap_or_else(|| Path::new(".aniflow/recipes")))?;
    if parent.components().any(|value| matches!(value,Component::CurDir|Component::ParentDir)) { return Err(invalid("recipe output parent must not contain traversal")); }
    for input in [&request.source_video,&request.frames_directory,&request.frame_index,&request.provider_executable]
        .into_iter().chain(intent.payload.components.iter().map(|component| &component.path)) {
        if parent.starts_with(input) { return Err(invalid("recipe output parent overlaps an input or selected component")); }
    }
    let mut ancestor = parent.as_path();
    while !ancestor.exists() { ancestor = ancestor.parent().ok_or_else(|| invalid("recipe output parent has no existing ancestor"))?; }
    require_existing_canonical(ancestor)?;
    if !ancestor.is_dir() { return Err(invalid("recipe output ancestor must be a directory")); }
    fs::create_dir_all(&parent).map_err(|_| state("cannot create recipe output parent"))?;
    require_existing_canonical(&parent)?;
    let private = tempfile::Builder::new().prefix("music-video-").tempdir_in(&parent).map_err(|_| state("cannot exclusively allocate recipe workspace"))?;
    let root = private.path();
    fs::create_dir(root.join("children")).map_err(|_| state("cannot initialize recipe child locators"))?;
    let bytes = canonical_json_bytes(intent)?;
    if bytes.len() as u64 > MAXIMUM_DOCUMENT_BYTES { return Err(invalid("recipe intent exceeds its 64 MiB bound")); }
    let mut file = OpenOptions::new().write(true).create_new(true).open(root.join("intent.json")).map_err(|_| state("cannot publish immutable recipe intent"))?;
    file.write_all(&bytes).and_then(|()| file.sync_all()).map_err(|_| state("cannot synchronize recipe intent"))?;
    for directory in [root.join("children"),root.to_path_buf(),parent] {
        fs::File::open(directory).and_then(|file| file.sync_all()).map_err(|_| state("cannot synchronize recipe workspace"))?;
    }
    Ok(private.keep())
}

pub(super) fn retained_plan(child: &Path) -> Result<crate::PipelineV3Plan> {
    let workspace = crate::PipelineV3Workspace::open_read_only(child).map_err(|_| state("recipe child has incomplete initialization; preserve its directory and reconcile it explicitly"))?;
    let bytes = read_bytes(&workspace.plan()).map_err(|_| state("recipe child has no intact published plan; preserve incomplete initialization for explicit reconciliation"))?;
    crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|failure| Error::new(failure.category(),failure.message))
}
