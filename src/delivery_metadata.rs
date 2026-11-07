//! Selected descriptive-tag policy for the fixed MP4 delivery profile.
//! Required codec, clock, geometry and color checks stay with media acceptance.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::{Error, ErrorCategory, Result};
pub mod mp4;
pub mod remux;

pub const DELIVERY_METADATA_POLICY_V1: &str = "aniflow.delivery-metadata-policy/v1";
fn invalid(message: &str) -> Error { Error::new(ErrorCategory::Media,message) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataMode { Preserve, Minimal, ClearInheritedThenApplyAllowlist }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseField { Title, Creator, Copyright, Credits, ProjectId, ReleaseId, Version, PublicUrl, Description }
impl ReleaseField {
    fn tag(self) -> &'static str { match self {
        Self::Title => "title", Self::Creator => "artist", Self::Copyright => "copyright", Self::Credits => "comment",
        Self::ProjectId => "project_id", Self::ReleaseId => "release_id", Self::Version => "version",
        Self::PublicUrl => "purl", Self::Description => "description",
    } }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryMetadataPolicy {
    pub schema: String,
    pub mode: MetadataMode,
    pub approved: BTreeMap<ReleaseField,String>,
}
impl DeliveryMetadataPolicy {
    pub fn validate(&self) -> Result<()> {
        if self.schema != DELIVERY_METADATA_POLICY_V1
            || (self.mode != MetadataMode::ClearInheritedThenApplyAllowlist && !self.approved.is_empty()) {
            return Err(invalid("metadata policy schema or approved-field mode is unsupported"));
        }
        let mut total = 0;
        for (field,value) in &self.approved {
            total += value.len();
            if value.trim().is_empty() || value.len() > 2048 || value.chars().any(char::is_control) {
                return Err(invalid("approved metadata must contain bounded printable operator-selected text"));
            }
            if *field == ReleaseField::PublicUrl && (!value.starts_with("https://")
                || value[8..].is_empty() || value.chars().any(char::is_whitespace)
                || value[8..].split('/').next().is_none_or(|host| host.contains('@') || host.is_empty())) {
                return Err(invalid("approved public URL requires an explicit HTTPS URL without user information"));
            }
        }
        if total > 8192 { return Err(invalid("approved metadata exceeds its aggregate text bound")); }
        Ok(())
    }
}

/// Private production evidence: tag values must never be copied into a routine
/// log or public artifact bundle. The public summary has no tag names/values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataInventory {
    pub global: BTreeMap<String,String>,
    pub streams: BTreeMap<u32,BTreeMap<String,String>>,
    pub chapters: Vec<BTreeMap<String,String>>,
    pub programs: Vec<BTreeMap<String,String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataInspectionStatus { Unsupported, CleanWithinScope, ResidualFields, Failed }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataPublicSummary {
    pub schema: String,
    pub mode: MetadataMode,
    pub status: MetadataInspectionStatus,
    pub before_fields: u64,
    pub after_fields: u64,
    pub residual_fields: u64,
    pub regenerated_fields: u64,
    pub inspected_scope: String,
    pub unsupported_scope: String,
    pub source_credentials: String,
    pub signal_marks: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataPrivateReceipt {
    pub schema: String,
    pub source_sha256: String,
    pub candidate_sha256: String,
    pub effective_policy: DeliveryMetadataPolicy,
    pub before: MetadataInventory,
    pub after: MetadataInventory,
    pub public_summary: MetadataPublicSummary,
}

fn tags(value: Option<&Value>, total: &mut usize) -> Result<BTreeMap<String,String>> {
    let Some(value) = value else { return Ok(BTreeMap::new()); };
    let object = value.as_object().ok_or_else(|| invalid("metadata tag scope is malformed"))?;
    if object.len() > 256 { return Err(invalid("metadata tag scope exceeds its field bound")); }
    let mut result = BTreeMap::new();
    for (name,value) in object {
        let value = value.as_str().ok_or_else(|| invalid("metadata tag value is not text"))?;
        *total = total.checked_add(name.len()+value.len()).ok_or_else(|| invalid("metadata size overflow"))?;
        if name.is_empty() || name.len() > 256 || value.len() > 8192 || *total > 65_536 {
            return Err(invalid("metadata inventory exceeds its private evidence bound"));
        }
        result.insert(name.clone(),value.to_owned());
    }
    Ok(result)
}

pub fn inventory(document: &Value) -> Result<MetadataInventory> {
    let mut total = 0;
    let format = document.get("format").and_then(Value::as_object).ok_or_else(|| invalid("metadata format inventory is missing"))?;
    let global = tags(format.get("tags"),&mut total)?;
    let values = document.get("streams").and_then(Value::as_array).ok_or_else(|| invalid("metadata stream inventory is missing"))?;
    if values.len() > 64 { return Err(invalid("metadata stream inventory exceeds its bound")); }
    let mut streams = BTreeMap::new();
    for value in values {
        let index = value.get("index").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| invalid("metadata stream identity is missing"))?;
        if streams.insert(index,tags(value.get("tags"),&mut total)?).is_some() { return Err(invalid("metadata stream identity is duplicated")); }
    }
    let mut scoped = |field: &str| -> Result<Vec<BTreeMap<String,String>>> {
        let values = document.get(field).and_then(Value::as_array).ok_or_else(|| invalid("metadata chapter/program scope was not inspected"))?;
        if values.len() > 256 { return Err(invalid("metadata chapter/program scope exceeds its bound")); }
        values.iter().map(|v| tags(v.get("tags"),&mut total)).collect()
    };
    Ok(MetadataInventory { global,streams,chapters:scoped("chapters")?,programs:scoped("programs")? })
}

fn push(args: &mut Vec<OsString>, flag: &str, value: impl Into<OsString>) { args.push(flag.into()); args.push(value.into()); }

/// Append output options before the output locator. Input mappings are explicit
/// and chapter treatment is separate. Does not invoke FFmpeg or grant acceptance.
pub fn append_arguments(args: &mut Vec<OsString>, policy: &DeliveryMetadataPolicy, before: &MetadataInventory,
    source_input: u32, video: u32, audio: Option<u32>) -> Result<()> {
    policy.validate()?;
    if !before.streams.contains_key(&video) || audio.is_some_and(|i| !before.streams.contains_key(&i)) { return Err(invalid("metadata selection does not match inspected source streams")); }
    if !before.programs.is_empty() { return Err(invalid("program metadata is outside the selected MP4 profile")); }
    match policy.mode {
        MetadataMode::Preserve => {
            push(args,"-map_metadata",source_input.to_string());
            push(args,"-map_metadata:s:v:0",format!("{source_input}:s:{video}"));
            if let Some(audio) = audio { push(args,"-map_metadata:s:a:0",format!("{source_input}:s:{audio}")); }
            push(args,"-map_chapters",source_input.to_string());
        },
        MetadataMode::Minimal | MetadataMode::ClearInheritedThenApplyAllowlist => {
            push(args,"-map_metadata","-1");
            push(args,"-map_metadata:s:v:0","-1");
            if audio.is_some() { push(args,"-map_metadata:s:a:0","-1"); }
            push(args,"-map_chapters","-1");
            for (out,index) in [("v",Some(video)),("a",audio)] {
                if let Some(language) = index.and_then(|i| before.streams.get(&i)).and_then(|v| v.get("language")) {
                    if language.len() != 3 || !language.bytes().all(|v| v.is_ascii_lowercase()) {
                        return Err(invalid("stream language cannot be preserved by the supported MP4 language field"));
                    }
                    push(args,&format!("-metadata:s:{out}:0"),format!("language={language}"));
                }
            }
        },
    }
    for (field,value) in &policy.approved { push(args,"-metadata",format!("{}={value}",field.tag())); }
    push(args,"-movflags","+faststart+use_metadata_tags");
    Ok(())
}

fn count(value: &MetadataInventory) -> u64 {
    value.global.len() as u64 + value.streams.values().map(|v| v.len() as u64).sum::<u64>()
        + value.chapters.iter().chain(&value.programs).map(|v| v.len() as u64).sum::<u64>()
}

/// Inert copy-only primitive for a separately orchestrated metadata-only
/// candidate. The caller must pin tools/inputs, enforce resource bounds and
/// independently validate elementary payloads, exact timing and final bytes.
/// No source replacement, execution or success claim is made by this builder.
pub fn remux_arguments(source: &Path, candidate: &Path, policy: &DeliveryMetadataPolicy,
    before: &MetadataInventory, video: u32, audio: Option<u32>) -> Result<Vec<OsString>> {
    if !source.is_absolute() || !candidate.is_absolute() || source == candidate
        || source.components().chain(candidate.components()).any(|v| matches!(v,std::path::Component::ParentDir|std::path::Component::CurDir))
        || Some(video) == audio || !before.streams.contains_key(&video)
        || audio.is_some_and(|i| !before.streams.contains_key(&i)) {
        return Err(invalid("metadata remux requires distinct confined source/candidate locators and selected streams"));
    }
    let mut args: Vec<OsString> = ["-hide_banner","-nostdin","-v","error","-xerror","-n","-copyts","-copytb","1","-protocol_whitelist","file,pipe","-noautorotate","-i"].into_iter().map(OsString::from).collect();
    args.push(source.as_os_str().to_owned());
    push(&mut args,"-map",format!("0:{video}"));
    if let Some(audio) = audio { push(&mut args,"-map",format!("0:{audio}")); }
    push(&mut args,"-c","copy");
    push(&mut args,"-avoid_negative_ts","disabled");
    append_arguments(&mut args,policy,before,0,video,audio)?;
    push(&mut args,"-f","mp4"); args.push(candidate.as_os_str().to_owned());
    Ok(args)
}
fn generated(name: &str) -> bool { matches!(name,"encoder"|"major_brand"|"minor_version"|"compatible_brands"|"handler_name"|"vendor_id") }

/// Verifies only bounded FFprobe-visible descriptive fields. Existing media
/// acceptance independently checks codecs, payload/timing, color and decode.
/// Opaque atoms, signatures and signal marks remain explicitly unqualified.
pub fn verify(policy: &DeliveryMetadataPolicy, before: MetadataInventory, after: MetadataInventory,
    source_sha256: &str, candidate_sha256: &str, video: u32, audio: Option<u32>) -> Result<MetadataPrivateReceipt> {
    policy.validate()?;
    crate::provider::require_sha256(source_sha256,"metadata source digest")?;
    crate::provider::require_sha256(candidate_sha256,"metadata candidate digest")?;
    let mut residual = 0;
    let mut regenerated = 0;
    let mut failed = false;
    if after.streams.len() != 1+usize::from(audio.is_some()) || !after.streams.contains_key(&0) || (audio.is_some() && !after.streams.contains_key(&1)) || !after.programs.is_empty() { failed = true; }
    match policy.mode {
        MetadataMode::Preserve => {
            for (name,value) in &before.global {
                if !generated(name) && after.global.get(name) != Some(value) { failed = true; }
            }
            if before.chapters != after.chapters { failed = true; }
            for (output,input) in [(0,Some(video)),(1,audio)] {
                if let Some(expected) = input.and_then(|i| before.streams.get(&i)) {
                    for (key,value) in expected {
                        if !generated(key) && after.streams.get(&output).and_then(|v| v.get(key)) != Some(value) { failed = true; }
                    }
                }
            }
        },
        MetadataMode::Minimal | MetadataMode::ClearInheritedThenApplyAllowlist => {
            if !after.chapters.is_empty() { failed = true; }
            for (field,value) in &policy.approved {
                if after.global.get(field.tag()) != Some(value) { failed = true; }
            }
            for (key,_) in &after.global {
                if generated(key) { regenerated += 1; }
                else if !policy.approved.keys().any(|field| field.tag() == key) { residual += 1; }
            }
            for (output,input) in [(0,Some(video)),(1,audio)] {
                let expected = input.and_then(|i| before.streams.get(&i)).and_then(|v| v.get("language"));
                let actual = after.streams.get(&output).and_then(|v| v.get("language"));
                if (expected.is_some() && expected != actual) || (expected.is_none() && actual.is_some_and(|v| v != "und")) { failed = true; }
                if let Some(scope) = after.streams.get(&output) {
                    for (key,_) in scope {
                        if generated(key) { regenerated += 1; }
                        else if key != "language" { residual += 1; }
                    }
                }
            }
        },
    }
    let status = if failed { MetadataInspectionStatus::Failed } else if residual > 0 { MetadataInspectionStatus::ResidualFields } else { MetadataInspectionStatus::Unsupported };
    let public_summary = MetadataPublicSummary {
        schema:"aniflow.delivery-metadata-summary/v1".to_owned(),mode:policy.mode,status,
        before_fields:count(&before),after_fields:count(&after),residual_fields:residual,regenerated_fields:regenerated,
        inspected_scope:"ffprobe format, selected stream, chapter and program tag dictionaries".to_owned(),
        unsupported_scope:"opaque/container atoms, nested namespaces, attachments and format-aware sanitation qualification".to_owned(),
        source_credentials:"source retained privately; no valid credential for changed bytes is asserted".to_owned(),
        signal_marks:"not removed or verified by descriptive metadata policy".to_owned(),
    };
    Ok(MetadataPrivateReceipt { schema:"aniflow.delivery-metadata-receipt/v1".to_owned(),source_sha256:source_sha256.to_owned(),
        candidate_sha256:candidate_sha256.to_owned(),effective_policy:policy.clone(),before,after,public_summary })
}
