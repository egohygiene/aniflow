//! Value/name-free descriptive-field changes, never universal sanitation proof.
use super::{
    MetadataInspectionStatus, MetadataInventory, MetadataMode, MetadataPrivateReceipt, invalid,
};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditScope {
    Global,
    SelectedVideo,
    SelectedAudio,
    Chapters,
    Programs,
    UnselectedStreams,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldClass {
    ApprovedRelease,
    Language,
    GeneratedMuxer,
    SensitiveOrHistory,
    OtherDescriptive,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldChange {
    Added,
    Removed,
    Retained,
    Changed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditBucket {
    pub scope: AuditScope,
    pub class: FieldClass,
    pub change: FieldChange,
    pub field_count: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAuthority {
    SuppliedPrivateReceipt,
    RetainedNativeRemuxAcceptance,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataAudit {
    pub schema: String,
    pub source_sha256: String,
    pub candidate_sha256: String,
    pub mode: MetadataMode,
    pub reported_status: MetadataInspectionStatus,
    pub authority: AuditAuthority,
    pub buckets: Vec<AuditBucket>,
    pub scope_limit: String,
    pub source_credentials: String,
    pub signal_marks: String,
}
fn bounded(inventory: &MetadataInventory) -> Result<()> {
    if inventory.streams.len() > 256
        || inventory.chapters.len() > 256
        || inventory.programs.len() > 256
    {
        return Err(invalid("metadata audit inventory exceeds scope bounds"));
    }
    let mut total = 0usize;
    for tags in std::iter::once(&inventory.global)
        .chain(inventory.streams.values())
        .chain(inventory.chapters.iter())
        .chain(inventory.programs.iter())
    {
        if tags.len() > 256 {
            return Err(invalid("metadata audit scope exceeds field bounds"));
        }
        for (name, value) in tags {
            total = total
                .checked_add(name.len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or_else(|| invalid("metadata audit size overflow"))?;
            if name.is_empty() || name.len() > 256 || value.len() > 8192 || total > 65_536 {
                return Err(invalid(
                    "metadata audit exceeds private inventory byte bounds",
                ));
            }
        }
    }
    Ok(())
}
fn class(name: &str, receipt: &MetadataPrivateReceipt) -> FieldClass {
    if receipt
        .effective_policy
        .approved
        .keys()
        .any(|field| field.tag() == name)
    {
        return FieldClass::ApprovedRelease;
    }
    if name == "language" {
        return FieldClass::Language;
    }
    if super::generated(name) {
        return FieldClass::GeneratedMuxer;
    }
    let lower = name.to_ascii_lowercase();
    if [
        "location",
        "latitude",
        "longitude",
        "gps",
        "device",
        "serial",
        "account",
        "owner",
        "creation_time",
        "software",
        "make",
        "model",
    ]
    .iter()
    .any(|part| lower.contains(part))
    {
        FieldClass::SensitiveOrHistory
    } else {
        FieldClass::OtherDescriptive
    }
}
type Counts = BTreeMap<(AuditScope, FieldClass, FieldChange), u64>;
fn scope(
    counts: &mut Counts,
    kind: AuditScope,
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    receipt: &MetadataPrivateReceipt,
) {
    let keys: BTreeSet<_> = before.keys().chain(after.keys()).collect();
    for key in keys {
        let change = match (before.get(key), after.get(key)) {
            (None, Some(_)) => FieldChange::Added,
            (Some(_), None) => FieldChange::Removed,
            (Some(a), Some(b)) if a == b => FieldChange::Retained,
            _ => FieldChange::Changed,
        };
        *counts
            .entry((kind, class(key, receipt), change))
            .or_default() += 1;
    }
}
/// Pure redacted projection of supplied evidence. Even a supplied clean status
/// is labeled caller-supplied, not revalidated or independently qualified.
pub fn from_receipt(
    receipt: &MetadataPrivateReceipt,
    video: u32,
    audio: Option<u32>,
) -> Result<MetadataAudit> {
    if receipt.schema != "aniflow.delivery-metadata-receipt/v1" || Some(video) == audio {
        return Err(invalid(
            "metadata audit needs its supported receipt and distinct selected streams",
        ));
    }
    receipt.effective_policy.validate()?;
    bounded(&receipt.before)?;
    bounded(&receipt.after)?;
    crate::provider::require_sha256(&receipt.source_sha256, "audit source")?;
    crate::provider::require_sha256(&receipt.candidate_sha256, "audit candidate")?;
    if !receipt.before.streams.contains_key(&video)
        || audio.is_some_and(|id| !receipt.before.streams.contains_key(&id))
    {
        return Err(invalid("metadata audit selected source stream is missing"));
    }
    let mut counts = Counts::new();
    let empty = BTreeMap::new();
    scope(
        &mut counts,
        AuditScope::Global,
        &receipt.before.global,
        &receipt.after.global,
        receipt,
    );
    for (kind, input, output) in [
        (AuditScope::SelectedVideo, Some(video), 0),
        (AuditScope::SelectedAudio, audio, 1),
    ] {
        if let Some(input) = input {
            scope(
                &mut counts,
                kind,
                receipt.before.streams.get(&input).unwrap_or(&empty),
                receipt.after.streams.get(&output).unwrap_or(&empty),
                receipt,
            );
        }
    }
    for (kind, before, after) in [
        (
            AuditScope::Chapters,
            &receipt.before.chapters,
            &receipt.after.chapters,
        ),
        (
            AuditScope::Programs,
            &receipt.before.programs,
            &receipt.after.programs,
        ),
    ] {
        for i in 0..before.len().max(after.len()) {
            scope(
                &mut counts,
                kind,
                before.get(i).unwrap_or(&empty),
                after.get(i).unwrap_or(&empty),
                receipt,
            );
        }
    }
    for (id, tags) in &receipt.before.streams {
        if *id != video && Some(*id) != audio {
            scope(
                &mut counts,
                AuditScope::UnselectedStreams,
                tags,
                &empty,
                receipt,
            );
        }
    }
    for (id, tags) in &receipt.after.streams {
        if *id != 0 && (audio.is_none() || *id != 1) {
            scope(
                &mut counts,
                AuditScope::UnselectedStreams,
                &empty,
                tags,
                receipt,
            );
        }
    }
    Ok(MetadataAudit {schema:"aniflow.delivery-metadata-audit/v1".into(),source_sha256:receipt.source_sha256.clone(),candidate_sha256:receipt.candidate_sha256.clone(),
        mode:receipt.effective_policy.mode,reported_status:receipt.public_summary.status,authority:AuditAuthority::SuppliedPrivateReceipt,
        buckets:counts.into_iter().map(|((scope,class,change),field_count)|AuditBucket {scope,class,change,field_count}).collect(),
        scope_limit:"bounded descriptive dictionaries only; categories are heuristic, names/values omitted; structural playback, opaque atoms and platform survival not established by this projection".into(),
        source_credentials:"independent retained history; no credential authenticity established".into(),signal_marks:"not inspected, removed or verified".into()})
}
/// Read-only current accepted remux evidence. No new probe, decode or mux.
pub fn observe_remux(root: &Path) -> Result<MetadataAudit> {
    let report = super::remux::observe_accepted_report(root)?;
    let mut value = from_receipt(
        &report.media.metadata,
        report
            .operation
            .selection
            .video_stream
            .ok_or_else(|| invalid("accepted remux selected video missing"))?,
        report.operation.selection.audio_stream,
    )?;
    if super::remux::observe_accepted_report(root)? != report {
        return Err(invalid("metadata remux changed during redacted audit"));
    }
    value.authority = AuditAuthority::RetainedNativeRemuxAcceptance;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt() -> MetadataPrivateReceipt {
        let before = MetadataInventory {
            global: BTreeMap::from([
                (
                    "private_CANARY_account".into(),
                    "SECRET_CANARY_VALUE".into(),
                ),
                ("title".into(), "old".into()),
            ]),
            streams: BTreeMap::from([(3, BTreeMap::from([("language".into(), "eng".into())]))]),
            chapters: Vec::new(),
            programs: Vec::new(),
        };
        let after = MetadataInventory {
            global: BTreeMap::from([
                ("title".into(), "new".into()),
                ("encoder".into(), "mux".into()),
            ]),
            streams: BTreeMap::from([(0, BTreeMap::from([("language".into(), "eng".into())]))]),
            chapters: Vec::new(),
            programs: Vec::new(),
        };
        let policy = super::super::DeliveryMetadataPolicy {
            schema: super::super::DELIVERY_METADATA_POLICY_V1.into(),
            mode: MetadataMode::ClearInheritedThenApplyAllowlist,
            approved: BTreeMap::from([(super::super::ReleaseField::Title, "new".into())]),
        };
        super::super::verify(
            &policy,
            before,
            after,
            &"a".repeat(64),
            &"b".repeat(64),
            3,
            None,
        )
        .unwrap()
    }
    #[test]
    fn public_projection_contains_no_private_tag_names_values_or_value_digests() {
        let r = receipt();
        let output = serde_json::to_string(&from_receipt(&r, 3, None).unwrap()).unwrap();
        assert!(!output.contains("CANARY"));
        assert!(!output.contains("old"));
        assert!(!output.contains("\"new\""));
    }
    #[test]
    fn removal_rewrite_generated_and_language_retention_are_distinct() {
        let audit = from_receipt(&receipt(), 3, None).unwrap();
        for (class, change) in [
            (FieldClass::SensitiveOrHistory, FieldChange::Removed),
            (FieldClass::ApprovedRelease, FieldChange::Changed),
            (FieldClass::GeneratedMuxer, FieldChange::Added),
            (FieldClass::Language, FieldChange::Retained),
        ] {
            assert!(
                audit
                    .buckets
                    .iter()
                    .any(|b| b.class == class && b.change == change && b.field_count == 1)
            );
        }
    }
    #[test]
    fn arbitrary_receipt_does_not_gain_current_native_acceptance() {
        assert_eq!(
            from_receipt(&receipt(), 3, None).unwrap().authority,
            AuditAuthority::SuppliedPrivateReceipt
        );
    }
    #[test]
    fn unknown_receipt_or_missing_ambiguous_streams_refuse() {
        let mut r = receipt();
        assert!(from_receipt(&r, 8, None).is_err());
        assert!(from_receipt(&r, 3, Some(3)).is_err());
        r.schema = "future".into();
        assert!(from_receipt(&r, 3, None).is_err());
    }
    #[test]
    fn direct_deserialization_cannot_bypass_inventory_bounds() {
        let mut r = receipt();
        r.before.global.insert("oversized".into(), "x".repeat(8193));
        assert!(from_receipt(&r, 3, None).is_err());
    }
}
