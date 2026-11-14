#!/usr/bin/env python3
"""Observe exact-final-video QC and compare explicitly synthetic dated requirements.

Existing native owners retain acceptance. No platform policy research, musical
approval, normalization, publication or descendant-cancellation qualification.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import sys
import time

from corpus.process import execute, require_passed, tree_bytes
from corpus.storage import regular_bytes, safe_new_directory

SPEC = importlib.util.spec_from_file_location("publishing_base", Path(__file__).with_name("publishing-pilot.py"))
base = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(base)
ROOT = Path(__file__).resolve().parents[1]
MAXIMUM = 512 * 1024 * 1024
REPORT_MAXIMUM = 131072


def snapshot(root):
    """Bounded exact inventory, including the supplied lyrics and accepted videos."""
    records, total, files, pending = {}, 0, 0, [root]
    while pending:
        with os.scandir(pending.pop()) as entries:
            for entry in entries:
                base.require(len(records) < 4096, "input inventory exceeds its entry-count bound")
                base.require(not entry.is_symlink(), "input inventory contains a symbolic member")
                path = Path(entry.path)
                relative = path.relative_to(root).as_posix()
                if entry.is_dir(follow_symlinks=False):
                    records[relative] = {"kind": "directory"}
                    pending.append(path)
                else:
                    base.require(entry.is_file(follow_symlinks=False), "input inventory contains a nonregular member")
                    record = base.file_record(path, base.MAX_TOOL)
                    total += record["bytes"]
                    files += 1
                    base.require(total <= MAXIMUM, "input inventory exceeds its byte bound")
                    records[relative] = {"kind": "file", **record}
    return {"files": files, "entries": len(records), "bytes": total,
            "sha256": hashlib.sha256(base.json_bytes(records)).hexdigest()}


def finite_measurement(measurement, unit):
    value = measurement.get("value", {})
    number = value.get("value")
    base.require(measurement.get("unit") == unit and value.get("kind") == "measured"
                 and type(number) in (int, float) and math.isfinite(number),
                 "unavailable, nonfinite or wrong-unit loudness is not measured evidence")
    return number


def observe_qc(value, expected_sha256, expected_bytes):
    base.require(value.get("schema") == "aniflow.final-audio-qc/v1"
                 and value.get("normalization") == "none"
                 and value.get("runtime_qualified") is False
                 and value.get("creative_approval") == "not_performed; measurements are not musical quality review"
                 and value.get("publication_authority") == "none", "QC authority or normalization changed")
    signal = value["signal"]
    pcm = value["accepted_pcm"]["source"]
    loudness = signal["loudness"]
    base.require(value["intent"]["artifact"]["sha256"] == expected_sha256
                 and signal["source_sha256"] == expected_sha256
                 and value["intent"]["artifact"]["byte_count"] == signal["source_bytes"] == expected_bytes
                 and signal["source_audio_extent"] == {"start": {"numerator": 0, "denominator": 1},
                                                        "end": {"numerator": 8, "denominator": 3}}
                 and signal["frames"] == pcm["frame_count"] == loudness["frames"] == 128000
                 and signal["sample_rate_hz"] == pcm["sample_rate_hz"] == loudness["sample_rate_hz"] == 48000
                 and signal["channels"] == pcm["channels"] == loudness["channels"] == 2
                 and pcm["artifact"]["sha256"] == signal["decoded_wav_sha256"]
                 and signal["decoded_pcm_sha256"] == loudness["pcm_sha256"]
                 and signal["normalization"] == "none" and signal["runtime_qualified"] is False
                 and loudness["runtime_qualified"] is False,
                 "QC no longer binds the selected complete stereo sample subject")
    return {"source_sha256": expected_sha256, "frames": 128000, "sample_rate_hz": 48000,
            "channels": 2, "decoded_pcm_sha256": signal["decoded_pcm_sha256"],
            "integrated_lufs": finite_measurement(loudness["integrated_loudness"], "loudness_units_full_scale"),
            "true_peak_dbtp": finite_measurement(loudness["true_peak"], "decibels_true_peak"),
            "normalization": "none", "native_meter_qualified": False}


def synthetic_preflight(owner, artifact, qc_directory, pin):
    return {"schema": "aniflow.destination-preflight-request/v1",
        "input": {"kind": "accepted_video_with_qc", "run_directory": str(owner),
                  "artifact_id": artifact, "qc_directory": str(qc_directory), "qc_receipt_sha256": pin},
        "evaluated_on": "2026-11-14", "profile": {
            "schema": "aniflow.destination-profile/v1", "id": "synthetic_cpu_qc_fixture",
            "revision": "fixture.v1", "destination": "tiktok", "route": "manual",
            "evidence": {"source_urls": ["https://developers.tiktok.com/doc/content-posting-api-media-transfer-guide"],
                "checked_on": "2026-11-14", "valid_through": "2026-11-15",
                "checked_by": "synthetic fixture only; supplied numbers are not platform requirements"},
            "maximum_file_bytes": 33554432, "maximum_duration": {"numerator": 8, "denominator": 3},
            "require_loudness_evidence": True, "requirements": {"kind": "video",
                "minimum_axis": 2, "maximum_axis": 4096, "required_width": 160, "required_height": 96,
                "minimum_frame_rate": {"numerator": 24, "denominator": 1},
                "maximum_frame_rate": {"numerator": 24, "denominator": 1},
                "audio_required": True, "maximum_audio_channels": 2, "require_zero_origin": True,
                "require_matching_audio_extent": True}}}


def observe_preflight(value, request, disposition, check_id, check_state):
    payload = value.get("payload", {})
    base.require(value.get("schema") == "aniflow.destination-preflight/v1"
                 and payload.get("request") == request
                 and payload.get("disposition") == disposition
                 and payload.get("creative_approval") == "not_performed"
                 and payload.get("publication_authority") == "none"
                 and payload.get("requirement_authority") ==
                 "operator_supplied_dated_requirements; not independently verified live platform policy",
                 "preflight changed its exact comparison or authority contract")
    matches = [item for item in payload.get("checks", []) if item.get("id") == check_id]
    base.require(len(matches) == 1 and matches[0].get("state") == check_state,
                 "preflight did not observe the selected comparison/refusal reason")
    return [{"id": item["id"], "state": item["state"]} for item in payload["checks"]]


class Pilot:
    """Shared bounded transport/evidence for two fixed, separately invoked pilots."""
    def __init__(self, pilot_root, aniflow, output):
        self.pilot_root = Path(pilot_root).resolve(strict=True)
        base.require(self.pilot_root.is_dir() and not Path(pilot_root).is_symlink(), "canonical pilot directory required")
        self.output = safe_new_directory(output, ROOT)
        base.require(self.output not in self.pilot_root.parents and self.pilot_root not in self.output.parents,
                     "continuation output must be disjoint from the accepted pilot")
        self.base_report = base.read_json(self.pilot_root / "pilot-report.json")
        base.require(self.base_report.get("schema") == "aniflow.synthetic-publishing-pilot/v1"
                     and self.base_report.get("state") == "passed", "a passed synthetic pilot is required")
        original = base.machine_result(self.pilot_root / "logs/run-source-recipe.stdout", "video_music_video")
        assembly = [p for p in original["phases"] if p["phase"] == "assembly" and p["state"] == "complete"]
        base.require(len(assembly) == 1, "one accepted assembly owner required")
        self.owner = Path(assembly[0]["run_directory"]).resolve(strict=True)
        base.require(self.pilot_root in self.owner.parents, "assembly owner escaped the pilot")
        self.configuration = base.read_json(self.pilot_root / "source-recipe.json")["recipe"]["assembly"]
        self.aniflow = Path(aniflow).resolve(strict=True)
        self.tools = {"aniflow": self.aniflow, **{n: Path(self.configuration[n]["executable"]).resolve(strict=True)
                                                 for n in ("ffmpeg", "ffprobe")}}
        for name in ("ffmpeg", "ffprobe"):
            base.require(self.configuration[name]["version"] == "8.0.1"
                         and base.file_record(self.tools[name], base.MAX_TOOL)["sha256"] == self.configuration[name]["sha256"],
                         "original explicit FFmpeg 8.0.1 pin changed")
        self.tool_records = {p: base.file_record(p, base.MAX_TOOL) for p in self.tools.values()}
        original_provider = Path(next(s["argv"][0] for s in self.base_report["steps"] if s["name"] == "version-aniflow"))
        base.require(base.file_record(original_provider, base.MAX_TOOL) ==
                     {k: self.base_report["tools"]["aniflow"][k] for k in ("bytes", "sha256")},
                     "accepted pilot provider changed")
        self.tool_records[original_provider] = base.file_record(original_provider, base.MAX_TOOL)
        self.initial = snapshot(self.pilot_root)
        self.output.mkdir(mode=0o700)
        self.started = time.monotonic()
        self.deadline = self.started + 120
        self.report = {"schema": "aniflow.synthetic-final-qc-pilot/v1", "state": "failed",
            "failure": None, "steps": [], "baseline": self.initial,
            "baseline_report_sha256": base.file_record(self.pilot_root / "pilot-report.json")["sha256"],
            "harness": base.file_record(Path(__file__)),
            "tools": {n: self.tool_records[p] for n, p in self.tools.items()},
            "limits": {"cli_seconds": 120, "monitored_output_bytes": MAXIMUM},
            "scope": "synthetic exact-subject technical observation only",
            "human_review_performed": False, "normalization_performed": False,
            "platform_policy_verified": False, "publication_performed": False,
            "release_qualified": False, "descendant_cancellation_qualified": False}

    def command(self, name, command, args, refusal=None):
        step = execute([self.aniflow, "--output", "json", *args], cwd=self.output,
                       logs=self.output / "logs", name=name, deadline=self.deadline,
                       environment=dict(os.environ), budget_root=self.output,
                       maximum_bytes=MAXIMUM - REPORT_MAXIMUM)
        self.report["steps"].append(step)
        if refusal is None:
            require_passed(step)
            return base.machine_result(self.output / "logs" / (name + ".stdout"), command)
        base.require(step["state"] == "failed" and step["exit_code"] not in (None, 0),
                     "expected a structured owner refusal")
        error = base.machine_error(self.output / "logs" / (name + ".stderr"), command)
        base.require(error.get("category") == "state" and error.get("message") == refusal,
                     "operation failed for an unrelated reason")
        return error

    def write(self, name, value):
        path = self.output / name
        path.write_bytes(base.json_bytes(value))
        return path

    def preserve(self):
        base.require(snapshot(self.pilot_root) == self.initial, "accepted pilot/source/lyrics inventory changed")
        for path, expected in self.tool_records.items():
            base.require(base.file_record(path, base.MAX_TOOL) == expected, "selected/original provider or tool changed")

    def finish(self):
        try:
            self.preserve()
            self.report["observed_payload_bytes"] = tree_bytes(self.output, MAXIMUM - REPORT_MAXIMUM)
        except (ValueError, OSError) as error:
            self.report.update(state="failed", failure=str(error)[:2000])
        self.report["elapsed_seconds"] = round(time.monotonic() - self.started, 3)
        encoded = base.json_bytes(self.report)
        base.require(len(encoded) <= REPORT_MAXIMUM, "bounded report exceeds 128 KiB")
        with (self.output / "qc-pilot-report.json").open("xb") as stream:
            stream.write(encoded)


def run(pilot_root, aniflow, output):
    pilot = Pilot(pilot_root, aniflow, output)
    try:
        request = {"schema": "aniflow.final-audio-qc-request/v1", "source_run_directory": str(pilot.owner),
            "artifact_id": "h264_delivery", "configuration": pilot.configuration,
            "provider_executable": str(pilot.aniflow), "maximum_pcm_bytes": 2097152}
        request_path = pilot.write("qc-request.json", request)
        plan = pilot.command("qc-plan", "delivery_audio_qc_plan", ["delivery", "audio-qc", "plan", "--request", request_path])
        base.require(plan["request"] == request and plan["artifact"]["artifact_id"] == "h264_delivery",
                     "QC plan changed the exact selected request/owner")
        qc_root = pilot.output / "qc"
        qc = pilot.command("qc-run", "delivery_audio_qc_run", ["delivery", "audio-qc", "run", "--request", request_path,
                                                               "--output-directory", qc_root])
        base.require(qc["intent"] == plan and base.read_json(qc_root / "qc.json") == qc, "QC plan/marker differs from observed report")
        expected = pilot.base_report["outputs"]["h264_delivery"]
        measurement = observe_qc(qc, expected["sha256"], expected["bytes"])
        pin = base.file_record(qc_root / "qc.json")["sha256"]
        qc_snapshot = snapshot(qc_root)
        status = pilot.command("qc-status", "delivery_audio_qc_status", ["delivery", "audio-qc", "status",
            "--directory", qc_root, "--receipt-sha256", pin])
        base.require(status == qc, "QC readback differs")
        pilot.command("refuse-wrong-qc-pin", "delivery_audio_qc_status", ["delivery", "audio-qc", "status",
            "--directory", qc_root, "--receipt-sha256", "0" * 64],
            refusal="final audio QC receipt differs from the external digest")
        profile = synthetic_preflight(pilot.owner, "h264_delivery", qc_root, pin)
        comparisons = {}
        for name, candidate, disposition, check_id, state in (
            ("matching", profile, "matches_supplied_profile", "loudness_evidence", "matches"),
            ("expired", {**copy.deepcopy(profile), "evaluated_on": "2026-11-16"}, "missing_evidence", "requirement_dates", "missing_evidence"),
            ("missing-qc", {**copy.deepcopy(profile), "input": {"kind": "accepted_video",
                "run_directory": str(pilot.owner), "artifact_id": "h264_delivery"}}, "missing_evidence", "loudness_evidence", "missing_evidence"),
            ("too-small", {**copy.deepcopy(profile), "profile": {**copy.deepcopy(profile["profile"]), "maximum_file_bytes": 1}},
                "needs_changes", "file_size", "needs_changes")):
            path = pilot.write("preflight-" + name + ".json", candidate)
            result = pilot.command("preflight-" + name, "delivery_preflight", ["delivery", "preflight", "--request", path])
            comparisons[name] = {"disposition": disposition, "checks": observe_preflight(result, candidate, disposition, check_id, state)}
        base.require(snapshot(qc_root) == qc_snapshot, "readback/preflight changed retained QC evidence")
        pilot.preserve()
        pilot.report.update(state="passed", measurement=measurement, qc_receipt_sha256=pin,
            qc_directory=str(qc_root), source_run_directory=str(pilot.owner), source_artifact="h264_delivery",
            baseline_directory=str(pilot.pilot_root), comparisons=comparisons, qc_readback_preserved=True,
            wrong_receipt_pin_refused=True, original_inventory_preserved=True)
    except (ValueError, OSError, KeyError, TypeError, ZeroDivisionError) as error:
        pilot.report["failure"] = str(error)[:2000]
    finally:
        pilot.finish()
    return pilot.report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pilot-root", type=Path, required=True)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        report = run(args.pilot_root, args.aniflow, args.output)
    except (ValueError, OSError) as error:
        print(str(error), file=sys.stderr)
        return 2
    print(json.dumps({"state": report["state"], "report": str(args.output / "qc-pilot-report.json"), "release_qualified": False}))
    return 0 if report["state"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
