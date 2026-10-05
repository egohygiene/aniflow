//! Pure parsers for the fixed FFmpeg/ffprobe query profiles.
//!
//! Formatting evidence: FFmpeg/FFmpeg at
//! ef52e1cc3850846987edc792c9583103e977e3b2: fftools/opt_common.c
//! (print_program_info, print_codecs, show_filters), fftools/ffmpeg_opt.c
//! (show_hwaccels, show_help_default), fftools/cmdutils.c
//! (show_help_options), fftools/opt_common.h (help aliases), and
//! fftools/ffprobe.c (show_help_default).
//! The earlier three-column filter rows are also source-backed by
//! b08d7969c550a804a59511c7b83f2dd8cc0499b8 (n7.1), opt_common.c.
//! Advertised build features do not establish device availability or a working
//! codec/filter invocation. No native qualification follows from these facts.

use std::collections::BTreeSet;

use super::probe_types::{
    ToolchainProbeCommandOutcome, ToolchainProbeCommandResult, ToolchainProbeProfile,
    ToolchainProbeTool, ToolchainProbeVersion, ToolchainVersionNormalization, invalid,
};
use super::types::ToolchainObservation;
use crate::Result;

pub(super) struct ToolchainParsedProbe {
    pub observation: ToolchainObservation,
    pub version: ToolchainProbeVersion,
}

pub(super) fn probe_commands(profile: ToolchainProbeProfile) -> Vec<(String, Vec<String>)> {
    let mut commands = vec![("version".to_owned(), vec!["-version".to_owned()])];
    if profile == ToolchainProbeProfile::Ffmpeg {
        for name in ["encoders", "decoders", "filters", "hwaccels"] {
            commands.push((name.to_owned(), vec!["-hide_banner".to_owned(), format!("-{name}")]));
        }
    }
    commands.push(("help_full".to_owned(), vec!["-hide_banner".to_owned(), "-h".to_owned(), "full".to_owned()]));
    commands
}

pub(super) fn parse_probe_observation(
    tool: &ToolchainProbeTool,
    commands: &[ToolchainProbeCommandResult],
    observed_sha256: &str,
) -> Result<ToolchainParsedProbe> {
    crate::provider::require_sha256(observed_sha256, "probe observed executable SHA-256")?;
    if observed_sha256 != tool.expected_sha256 {
        return Err(invalid("probe parser refuses executable bytes outside the requested pin"));
    }
    let expected = probe_commands(tool.profile);
    if commands.len() != expected.len() {
        return Err(invalid("parsed tool observations require every fixed query"));
    }
    let mut outputs = Vec::with_capacity(commands.len());
    for (command, (id, arguments)) in commands.iter().zip(&expected) {
        if command.command_id != *id || command.arguments != *arguments
            || command.outcome != ToolchainProbeCommandOutcome::Succeeded
            || command.exit_code != Some(0) || command.signal.is_some()
            || command.stdout.truncated || command.stderr.truncated
        {
            return Err(invalid("probe parser requires exact successful commands and complete captures"));
        }
        // Refuse undecodable output, even on the diagnostic stream. Facts come
        // only from stdout, the upstream query output channel.
        let stdout = command.stdout.text()?;
        let stderr = command.stderr.text()?;
        if [&stdout, &stderr].into_iter().any(|text| text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))) {
            return Err(invalid("probe output contains unsupported control characters"));
        }
        outputs.push(stdout);
    }
    let version = parse_version(tool.profile, &outputs[0])?;
    let mut features = BTreeSet::new();
    if tool.profile == ToolchainProbeProfile::Ffmpeg {
        codec_features(&outputs[1], "Encoders:", "encoder", &mut features)?;
        codec_features(&outputs[2], "Decoders:", "decoder", &mut features)?;
        filter_features(&outputs[3], &mut features)?;
        hardware_features(&outputs[4], &mut features)?;
    }
    let flags = help_flags(tool.profile, outputs.last().ok_or_else(|| invalid("missing help capture"))?)?;
    if flags.len() > 4096 || features.len() > 4096 {
        return Err(invalid("probe observations exceed 4096 advertised flags or features"));
    }
    let normalization = match version.normalization {
        Some(ToolchainVersionNormalization::Exact) => "exact",
        Some(ToolchainVersionNormalization::AppendPatchZero) => "append_patch_zero",
        None => "unverifiable",
    };
    let package_source = if tool.caller_package_revision.is_some() { "caller-supplied" } else { "absent" };
    // Bind reproducible process/capture facts, leaving wall-clock duration as
    // separately retained telemetry rather than changing observation identity.
    let evidence: Vec<_> = commands.iter().map(|command| (
        &command.command_id, &command.arguments, command.outcome,
        command.exit_code, command.signal, &command.stdout, &command.stderr,
    )).collect();
    let provenance = format!(
        "aniflow.toolchain-probe/v1; executable_sha256={observed_sha256}; commands_sha256={}; version_normalization={normalization}; package_revision={package_source}; hwaccels=compiled-backends-only; native_qualification=false",
        crate::provider::canonical_sha256(&evidence)?,
    );
    Ok(ToolchainParsedProbe {
        observation: ToolchainObservation {
            executable_sha256: observed_sha256.to_owned(),
            version: version.normalized_semver.clone(),
            package_revision: tool.caller_package_revision.clone(),
            native_scale: None,
            flags: flags.into_iter().collect(),
            features: features.into_iter().collect(),
            provenance,
        },
        version,
    })
}

fn parse_version(profile: ToolchainProbeProfile, output: &str) -> Result<ToolchainProbeVersion> {
    let program = match profile { ToolchainProbeProfile::Ffmpeg => "ffmpeg", ToolchainProbeProfile::Ffprobe => "ffprobe" };
    let mut lines = output.lines().filter(|line| !line.trim().is_empty());
    let first = lines.next().ok_or_else(|| invalid("missing tool version banner"))?;
    let prefix = format!("{program} version ");
    let remainder = first.strip_prefix(&prefix).ok_or_else(|| invalid("version banner does not identify the requested tool"))?;
    let (raw, copyright) = remainder.split_once(' ').ok_or_else(|| invalid("incomplete tool version banner"))?;
    if !copyright.starts_with("Copyright (c) ") || raw.is_empty() || raw.len() > 256
        || raw.chars().any(char::is_whitespace)
        || lines.any(|line| line.starts_with("ffmpeg version ") || line.starts_with("ffprobe version "))
    {
        return Err(invalid("ambiguous or malformed tool version banner"));
    }
    let components: Vec<_> = raw.split('.').collect();
    let numeric = matches!(components.len(), 2 | 3) && components.iter().all(|component| {
        !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
            && (component.len() == 1 || !component.starts_with('0'))
            && component.parse::<u64>().is_ok()
    });
    let (normalized_semver, normalization) = if numeric {
        if components.len() == 2 {
            (Some(format!("{raw}.0")), Some(ToolchainVersionNormalization::AppendPatchZero))
        } else {
            (Some(raw.to_owned()), Some(ToolchainVersionNormalization::Exact))
        }
    } else { (None, None) };
    Ok(ToolchainProbeVersion { raw_token: raw.to_owned(), normalized_semver, normalization })
}

fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn section_lines<'a>(output: &'a str, heading: &str) -> Result<Vec<&'a str>> {
    let mut lines = output.lines().filter(|line| !line.trim().is_empty());
    if lines.next() != Some(heading) {
        return Err(invalid(format!("probe output lacks the exact {heading} heading")));
    }
    Ok(lines.collect())
}

fn codec_features(output: &str, heading: &str, namespace: &str, features: &mut BTreeSet<String>) -> Result<()> {
    const LEGEND: &[&str] = &[
        "V..... = Video", "A..... = Audio", "S..... = Subtitle",
        ".F.... = Frame-level multithreading", "..S... = Slice-level multithreading",
        "...X.. = Codec is experimental", "....B. = Supports draw_horiz_band",
        ".....D = Supports direct rendering method 1", "------",
    ];
    for line in section_lines(output, heading)? {
        if LEGEND.contains(&line.trim()) { continue; }
        let mut columns = line.split_whitespace();
        let flags = columns.next().unwrap_or_default().as_bytes();
        let name = columns.next().unwrap_or_default();
        if !line.starts_with(' ') || flags.len() != 6 || !matches!(flags[0], b'V' | b'A' | b'S' | b'D' | b'T')
            || !flags[1..].iter().zip(b"FSXBD").all(|(actual, expected)| *actual == b'.' || actual == expected)
            || !identifier(name)
        {
            return Err(invalid(format!("unrecognized {namespace} advertisement row")));
        }
        if !features.insert(format!("{namespace}:{name}")) {
            return Err(invalid(format!("duplicate {namespace} advertisement")));
        }
    }
    Ok(())
}

fn filter_features(output: &str, features: &mut BTreeSet<String>) -> Result<()> {
    if output.trim() == "No filters available: libavfilter disabled" { return Ok(()); }
    const LEGEND: &[&str] = &[
        "T.. = Timeline support", ".S. = Slice threading", "..C = Command support",
        "A = Audio input/output", "V = Video input/output", "N = Dynamic number and/or type of input/output",
        "| = Source or sink filter", "------",
    ];
    for line in section_lines(output, "Filters:")? {
        if LEGEND.contains(&line.trim()) { continue; }
        let mut columns = line.split_whitespace();
        let flags = columns.next().unwrap_or_default().as_bytes();
        let name = columns.next().unwrap_or_default();
        let signature = columns.next().unwrap_or_default();
        let io = signature.split_once("->").is_some_and(|(input, output)| {
            [input, output].into_iter().all(|side| !side.is_empty() && side.len() <= 32
                && side.bytes().all(|byte| matches!(byte, b'A' | b'V' | b'S' | b'D' | b'T' | b'N' | b'|')))
        });
        if !line.starts_with(' ') || !matches!(flags.len(), 2 | 3)
            || !flags.iter().zip(b"TSC").all(|(actual, expected)| *actual == b'.' || actual == expected)
            || !identifier(name) || !io
        {
            return Err(invalid("unrecognized filter advertisement row"));
        }
        if !features.insert(format!("filter:{name}")) {
            return Err(invalid("duplicate filter advertisement"));
        }
    }
    Ok(())
}

fn hardware_features(output: &str, features: &mut BTreeSet<String>) -> Result<()> {
    for line in section_lines(output, "Hardware acceleration methods:")? {
        if !identifier(line) || !features.insert(format!("hwaccel:{line}")) {
            return Err(invalid("unrecognized or duplicate compiled hardware backend row"));
        }
    }
    Ok(())
}

fn help_flags(profile: ToolchainProbeProfile, output: &str) -> Result<BTreeSet<String>> {
    const FFMPEG_HEADINGS: &[&str] = &[
        "Print help / information / capabilities:", "Advanced information / capabilities:",
        "Global options (affect whole program instead of just one file):", "Advanced global options:",
        "Per-file options (input and output):", "Advanced per-file options (input and output):",
        "Per-file options (input-only):", "Advanced per-file options (input-only):",
        "Per-file options (output-only):", "Advanced per-file options (output-only):",
        "Per-stream options:", "Advanced per-stream options:", "Video options:", "Advanced Video options:",
        "Audio options:", "Advanced Audio options:", "Subtitle options:", "Advanced Subtitle options:",
        "Data options:", "Advanced Data options:", "Data stream options:",
        // Earlier release headings, before the per-file/per-stream grouping.
        "Main options:", "Advanced options:",
    ];
    let mut flags = BTreeSet::new();
    let mut in_section = false;
    let mut seen_heading = false;
    for line in output.lines() {
        let recognized = match profile {
            ToolchainProbeProfile::Ffmpeg => FFMPEG_HEADINGS.contains(&line),
            ToolchainProbeProfile::Ffprobe => line == "Main options:",
        };
        if recognized { in_section = true; seen_heading = true; continue; }
        if line.is_empty() { in_section = false; continue; }
        if !in_section { continue; }
        // Only the leading option field of an actual top-level help row counts.
        // Usage text, descriptions, suffix matches and AVOption child sections
        // cannot manufacture a top-level command-line flag.
        if !line.starts_with('-') { in_section = false; continue; }
        let Some(boundary) = line.find(char::is_whitespace) else {
            return Err(invalid("help option row lacks a description boundary"));
        };
        let token = &line[..boundary];
        let base = token.strip_suffix("[:<stream_spec>]")
            .or_else(|| token.strip_suffix("[:<spec>]"))
            .unwrap_or(token);
        let name = base.strip_prefix('-').unwrap_or_default();
        if !(identifier(name) || name == "?") || line[boundary..].trim().is_empty() {
            return Err(invalid("unrecognized top-level help option row"));
        }
        flags.insert(base.to_owned());
    }
    if !seen_heading || flags.is_empty() {
        return Err(invalid("help query lacks a recognized nonempty option section"));
    }
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_preserve_unknown_tokens_and_name_the_only_allowed_normalization() {
        let parse = |token| parse_version(ToolchainProbeProfile::Ffmpeg,
            &format!("ffmpeg version {token} Copyright (c) fixture\n"));
        assert_eq!(parse("7.1").unwrap().normalization, Some(ToolchainVersionNormalization::AppendPatchZero));
        assert_eq!(parse("7.1.2").unwrap().normalization, Some(ToolchainVersionNormalization::Exact));
        for token in ["N-123-gabcd", "7.1-custom", "n7.1", "07.1", "7.1.2.3", "7"] {
            let version = parse(token).unwrap();
            assert_eq!(version.raw_token, token);
            assert!(version.normalized_semver.is_none());
            assert!(version.normalization.is_none());
        }
        assert!(parse_version(ToolchainProbeProfile::Ffprobe,
            "ffmpeg version 7.1 Copyright (c) fixture\n").is_err());
        assert!(parse_version(ToolchainProbeProfile::Ffmpeg,
            "usage: ffmpeg version 7.1 Copyright (c) fixture\n").is_err());
    }

    #[test]
    fn codec_names_come_only_from_valid_advertisement_rows() {
        let mut features = BTreeSet::new();
        codec_features("Encoders:\n V..... libx264 description mentions pcm_s16le\n",
            "Encoders:", "encoder", &mut features).unwrap();
        assert_eq!(features, BTreeSet::from(["encoder:libx264".to_owned()]));
        for row in ["description libx264", " V..... = libx264", " ....V. libx264 fake", " V..... libx264, fake"] {
            assert!(codec_features(&format!("Encoders:\n{row}\n"), "Encoders:", "encoder", &mut BTreeSet::new()).is_err());
        }
    }

    #[test]
    fn exact_filter_and_compiled_backend_rows_do_not_assert_device_availability() {
        let mut features = BTreeSet::new();
        filter_features("Filters:\n  T.. = Timeline support\n .. scale V->V scale\n ... anull A->A pass\n", &mut features).unwrap();
        hardware_features("Hardware acceleration methods:\nvulkan\n", &mut features).unwrap();
        assert_eq!(features, BTreeSet::from([
            "filter:anull".to_owned(), "filter:scale".to_owned(), "hwaccel:vulkan".to_owned(),
        ]));
        assert!(filter_features("Filters:\n ... scale description_without_ports\n", &mut BTreeSet::new()).is_err());
        assert!(hardware_features("Hardware acceleration methods:\nvulkan available\n", &mut BTreeSet::new()).is_err());
    }

    #[test]
    fn help_flags_require_actual_option_boundaries_in_known_sections() {
        let flags = help_flags(ToolchainProbeProfile::Ffmpeg, concat!(
            "usage: ffmpeg -i INPUT\n",
            "Global options (affect whole program instead of just one file):\n",
            "-hide_banner         description mentions -unobserved\n",
            "-version_suffix      does not prove -version\n",
            "-? <topic>           show help\n",
            "--help <topic>       show help\n",
            "\nPer-stream options:\n",
            "-codec[:<stream_spec>] <codec>  codec selection\n",
            "\nPrivate AVOptions:\n",
            "  -private_child <int> description\n",
        )).unwrap();
        assert_eq!(flags, BTreeSet::from([
            "--help".to_owned(), "-?".to_owned(), "-codec".to_owned(),
            "-hide_banner".to_owned(), "-version_suffix".to_owned(),
        ]));
        assert!(help_flags(ToolchainProbeProfile::Ffmpeg,
            "usage: ffmpeg -version -i INPUT\n").is_err());
        assert!(help_flags(ToolchainProbeProfile::Ffprobe,
            "Main options:\n-show_entries_extra=x fake\n").is_err());
    }
}
