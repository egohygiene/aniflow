//! Schema selection for the exact managed runtime extension.
use super::*;

pub(super) fn schema(
    settings: &UpscaylRecoveryConfiguration,
    mut legacy: crate::ConfigurationSchemaReference,
) -> crate::ConfigurationSchemaReference {
    if settings.managed_runtime.is_none() {
        return legacy;
    }
    let bytes: &[u8] = match legacy.id.as_str() {
        UPSCAYL_FRAME_OPERATION_SCHEMA_V1 => {
            include_bytes!("../../providers/upscayl-recovery/frame-operation-managed.schema.json")
        }
        UPSCAYL_SELECTED_FRAME_OPERATION_SCHEMA_V2 => include_bytes!(
            "../../providers/upscayl-recovery/frame-operation-v2-managed.schema.json"
        ),
        UPSCAYL_COLLECT_OPERATION_SCHEMA_V1 => {
            include_bytes!("../../providers/upscayl-recovery/collect-operation-managed.schema.json")
        }
        UPSCAYL_CHUNK_OPERATION_SCHEMA_V1 => {
            include_bytes!("../../providers/upscayl-recovery/chunk-operation-managed.schema.json")
        }
        UPSCAYL_CHUNK_COLLECT_OPERATION_SCHEMA_V1 => include_bytes!(
            "../../providers/upscayl-recovery/chunk-collect-operation-managed.schema.json"
        ),
        _ => unreachable!("private managed schema selector only accepts existing Upscayl schemas"),
    };
    let (name, major) = legacy
        .id
        .rsplit_once("/v")
        .expect("known versioned Upscayl schema");
    legacy.id = format!("{name}-managed/v{major}");
    legacy.sha256 = format!("{:x}", Sha256::digest(bytes));
    legacy
}

pub(super) fn require_output_disjoint(
    settings: &UpscaylRecoveryConfiguration,
    path: &Path,
) -> Result<()> {
    if let Some(runtime) = &settings.managed_runtime {
        runtime.require_output_disjoint(path)?;
    }
    Ok(())
}
