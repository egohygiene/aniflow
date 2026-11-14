//! Side-effect-free source contract inventory, also retained in native archives.
//! This describes implemented interfaces; it never probes tools/models, grants
//! authority, asserts publication or qualifies a platform.
use serde::Serialize;

pub const RELEASE_CONTRACT_V1: &str = "aniflow.release-contract/v1";
#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub id: &'static str,
    pub input: &'static str,
    pub output: &'static str,
    pub plan_argv: &'static [&'static str],
    pub run_argv: &'static [&'static str],
    pub status_argv: &'static [&'static str],
    pub resume_argv: &'static [&'static str],
    pub acceptance_argv: &'static [&'static str],
    pub requirements: &'static [&'static str],
    pub source_mutation: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct ContractSchema {
    pub name: &'static str,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseContract {
    pub schema_version: &'static str,
    pub tool: &'static str,
    pub version: &'static str,
    pub machine_schema_version: u32,
    pub library_crate: &'static str,
    pub library_version: &'static str,
    pub inventory_scope: &'static str,
    pub profiles: Vec<Profile>,
    pub schemas: Vec<ContractSchema>,
    pub native_packaging_targets: &'static [&'static str],
    pub retained_platform_objectives: &'static [&'static str],
    pub qualification: &'static str,
    pub runtime_availability: &'static str,
}
fn processing_profile(id: &'static str, input: &'static str, output: &'static str) -> Profile {
    Profile {
        id,
        input,
        output,
        plan_argv: &[
            "--output",
            "json",
            "delivery",
            "audio-processing",
            "plan",
            "--request",
            "{configuration}",
        ],
        run_argv: &[
            "--output",
            "json",
            "delivery",
            "audio-processing",
            "run",
            "--request",
            "{configuration}",
            "--output-directory",
            "{output_parent}",
        ],
        status_argv: &[
            "--output",
            "json",
            "delivery",
            "audio-processing",
            "status",
            "--request",
            "{configuration}",
            "--run-directory",
            "{native_run}",
        ],
        resume_argv: &[
            "--output",
            "json",
            "delivery",
            "audio-processing",
            "resume",
            "--request",
            "{configuration}",
            "--run-directory",
            "{native_run}",
        ],
        acceptance_argv: &[
            "--output",
            "json",
            "delivery",
            "audio-processing",
            "status",
            "--request",
            "{configuration}",
            "--run-directory",
            "{native_run}",
        ],
        requirements: &[
            "exact current native source/QC/tool pins and explicit settings",
            "existing native acceptance and disjoint fresh outputs",
            "separate Flow adapter, meter/platform qualification and human review",
        ],
        source_mutation: false,
    }
}
/// No filesystem reads, native probes, model discovery or environment lookup.
#[must_use]
pub fn describe() -> ReleaseContract {
    use sha2::{Digest, Sha256};
    let schema = |name, bytes: &[u8]| ContractSchema {
        name,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    };
    ReleaseContract {
        schema_version: RELEASE_CONTRACT_V1,
        tool: "aniflow",
        version: env!("CARGO_PKG_VERSION"),
        machine_schema_version: crate::MACHINE_SCHEMA_VERSION,
        library_crate: "aniflow",
        library_version: env!("CARGO_PKG_VERSION"),
        inventory_scope: "initial_flow_profiles; other native commands remain independently available",
        profiles: vec![
            Profile {
                id: "music-video-source-v1",
                input: "source video + aniflow.source-music-video-recipe/v1",
                output: "accepted master and delivery with original rational timeline",
                plan_argv: &[
                    "--output",
                    "json",
                    "video",
                    "music-video",
                    "--source-video",
                    "{source}",
                    "--configuration",
                    "{configuration}",
                    "--plan",
                ],
                run_argv: &[
                    "--output",
                    "json",
                    "video",
                    "music-video",
                    "--source-video",
                    "{source}",
                    "--configuration",
                    "{configuration}",
                    "--output-directory",
                    "{output_parent}",
                ],
                status_argv: &[
                    "--output",
                    "json",
                    "video",
                    "music-video-status",
                    "{native_run}",
                ],
                resume_argv: &[
                    "--output",
                    "json",
                    "video",
                    "music-video",
                    "--source-video",
                    "{source}",
                    "--configuration",
                    "{configuration}",
                    "--resume-run-directory",
                    "{native_run}",
                ],
                acceptance_argv: &["--output", "json", "video", "completion", "{native_run}"],
                requirements: &[
                    "exact external FFmpeg/FFprobe and recipe tool/model pins",
                    "Unix native execution",
                    "fresh disjoint output parent",
                ],
                source_mutation: false,
            },
            Profile {
                id: "manual-timed-text-v1",
                input: "manually supplied timed lyrics (SRT or another supported timed format)",
                output: "explicit timed-text export",
                plan_argv: &[],
                run_argv: &[],
                status_argv: &[],
                resume_argv: &[],
                acceptance_argv: &[],
                requirements: &[
                    "explicit supplied timing and lyrics; no automatic recognition",
                    "use native timed-text/video lyrics contracts; not a Flow execution profile yet",
                ],
                source_mutation: false,
            },
            Profile {
                id: "accepted-final-video-v1",
                input: "accepted fixed-profile native run",
                output: "aniflow.final-video-handoff/v1",
                plan_argv: &[],
                run_argv: &[],
                status_argv: &[],
                resume_argv: &[],
                acceptance_argv: &["--output", "json", "video", "completion", "{native_run}"],
                requirements: &[
                    "fresh native domain acceptance; partial recipes cannot emit completion",
                ],
                source_mutation: false,
            },
            Profile {
                id: "audio-policy-check-v1",
                input: "exact final media + aniflow.audio-policy-check-request/v1",
                output: "aniflow.audio-policy-assessment/v1; numeric matching is not meter qualification or creative approval",
                plan_argv: &[
                    "--output",
                    "json",
                    "delivery",
                    "audio-policy",
                    "check",
                    "--request",
                    "{configuration}",
                ],
                run_argv: &[
                    "--output",
                    "json",
                    "delivery",
                    "audio-policy",
                    "check",
                    "--request",
                    "{configuration}",
                ],
                status_argv: &[
                    "--output",
                    "json",
                    "delivery",
                    "audio-policy",
                    "check",
                    "--request",
                    "{configuration}",
                ],
                resume_argv: &[
                    "--output",
                    "json",
                    "delivery",
                    "audio-policy",
                    "check",
                    "--request",
                    "{configuration}",
                ],
                acceptance_argv: &[
                    "--output",
                    "json",
                    "delivery",
                    "audio-policy",
                    "check",
                    "--request",
                    "{configuration}",
                ],
                requirements: &[
                    "current exact-final-file QC and selected policy",
                    "read-only current owner comparison; no tool/model processing or upload",
                    "separate meter/native qualification and human review",
                ],
                source_mutation: false,
            },
            processing_profile(
                "fixed-audio-gain-v1",
                "aniflow.audio-gain-request/v1",
                "accepted separate gain WAV; fresh QC required",
            ),
            processing_profile(
                "final-lossless-audio-qc-v1",
                "aniflow.resumable-final-audio-qc-request/v1",
                "fresh exact-final-lossless QC report; separate selected-policy check required",
            ),
            processing_profile(
                "final-video-audio-qc-v1",
                "aniflow.resumable-final-video-audio-qc-request/v1",
                "fresh exact-encoded-file QC report; separate selected-policy check required",
            ),
            processing_profile(
                "gain-video-v1",
                "aniflow.gain-video-request/v1",
                "accepted picture-copy/AAC candidate; fresh encoded QC required",
            ),
        ],
        schemas: vec![
            schema(
                "machine-envelope-v1",
                include_bytes!("../docs/contracts/machine-envelope-v1.schema.json"),
            ),
            schema(
                "final-video-handoff-v1",
                include_bytes!("../docs/contracts/final-video-handoff-v1.schema.json"),
            ),
            schema(
                "source-music-video-plan-v1",
                include_bytes!("../docs/contracts/source-music-video-plan-v1.schema.json"),
            ),
            schema(
                "source-music-video-outcome-v1",
                include_bytes!("../docs/contracts/source-music-video-outcome-v1.schema.json"),
            ),
            schema(
                "source-music-video-status-v1",
                include_bytes!("../docs/contracts/source-music-video-status-v1.schema.json"),
            ),
            schema(
                "audio-delivery-policy-v1",
                include_bytes!("../docs/contracts/audio-delivery-policy-v1.schema.json"),
            ),
            schema(
                "audio-policy-check-request-v1",
                include_bytes!("../docs/contracts/audio-policy-check-request-v1.schema.json"),
            ),
            schema(
                "audio-policy-assessment-v1",
                include_bytes!("../docs/contracts/audio-policy-assessment-v1.schema.json"),
            ),
            schema(
                "audio-processing-plan-v1",
                include_bytes!("../docs/contracts/audio-processing-plan-v1.schema.json"),
            ),
            schema(
                "audio-processing-handoff-v1",
                include_bytes!("../docs/contracts/audio-processing-handoff-v1.schema.json"),
            ),
            schema(
                "audio-gain-request-v1",
                include_bytes!("../docs/contracts/audio-gain-request-v1.schema.json"),
            ),
            schema(
                "gain-video-request-v1",
                include_bytes!("../docs/contracts/gain-video-request-v1.schema.json"),
            ),
        ],
        native_packaging_targets: &[
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
        ],
        retained_platform_objectives: &["x86_64-pc-windows-msvc"],
        qualification: "not_established_by_contract_description",
        runtime_availability: "requires_separate_current_tool_model_and_native_evidence",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn descriptor_is_deterministic_and_does_not_claim_a_qualified_release() {
        let first = serde_json::to_value(super::describe()).unwrap();
        assert_eq!(first, serde_json::to_value(super::describe()).unwrap());
        assert_eq!(
            first["qualification"],
            "not_established_by_contract_description"
        );
        assert_eq!(first["profiles"][0]["source_mutation"], false);
        assert!(
            first["retained_platform_objectives"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "x86_64-pc-windows-msvc")
        );
    }
}
