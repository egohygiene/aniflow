#!/usr/bin/env python3
"""Run metadata-only finishing, fresh final QC and exact stale-QC refusal.

Synthetic accepted inputs only. No captions, human decisions, normalization,
signing, platform policy verification or publication are supplied.
"""
from __future__ import annotations

import argparse
import copy
from fractions import Fraction
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys

from corpus.process import execute, require_passed

SPEC = importlib.util.spec_from_file_location("publishing_qc", Path(__file__).with_name("publishing-qc-pilot.py"))
qc = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qc)
base = qc.base
TITLE = "Synthetic metadata finishing checkpoint"


def video_identity(document):
    streams = [s for s in document["streams"] if s["codec_type"] == "video"]
    base.require(len(streams) == 1, "one video stream required")
    stream = streams[0]
    clock = Fraction(stream["time_base"])
    configuration = {k: stream[k] for k in ("codec_name", "profile", "level", "extradata_size",
                                           "extradata_hash", "is_avc", "nal_length_size")}
    base.require(re.fullmatch(r"SHA256:[0-9a-f]{64}", configuration["extradata_hash"]) is not None,
                 "video codec configuration lacks its observed digest")
    packets = []
    for packet in document["packets"]:
        if packet["stream_index"] != stream["index"]:
            continue
        base.require(re.fullmatch(r"SHA256:[0-9a-f]{64}", packet["data_hash"]) is not None,
                     "video packet lacks its observed digest")
        packets.append({**{key: str(Fraction(packet[key]) * clock) for key in ("pts", "dts", "duration")},
            **{key: packet[key] for key in ("size", "data_hash", "flags")},
            "side_data": packet.get("side_data_list", [])})
    base.require(len(packets) == 64, "complete 64-packet video observation required")
    return {"configuration": configuration, "packets": packets}


def observe_finishing(value):
    steps = value.get("steps", [])
    completion = value.get("completion", {})
    artifacts = completion.get("artifacts", []) if isinstance(completion, dict) else []
    base.require(isinstance(steps, list) and isinstance(completion, dict) and isinstance(artifacts, list),
                 "finishing status must contain typed steps and completion")
    base.require(value.get("schema") == "aniflow.finishing-status/v1"
                 and value.get("complete") is True and value.get("preview") is None and value.get("review") is None
                 and value.get("publication_authority") == "none"
                 and value.get("creative_approval") ==
                 "full_delivery_review_not_performed; preview_decision_only_authorizes_candidate_generation"
                 and len(steps) == 1 and steps[0].get("phase") == "metadata_remux" and steps[0].get("state") == "complete"
                 and completion.get("schema") == "aniflow.final-video-handoff/v1"
                 and completion.get("creative_approval") == "not_recorded_by_fixed_automated_profile"
                 and completion.get("publication_authority") == "none" and completion.get("proof_state") == "not_asserted"
                 and len(artifacts) == 1 and artifacts[0].get("artifact_id") == "metadata_delivery"
                 and artifacts[0].get("variant") == "destination_copy",
                 "metadata-only finishing changed its exact operation, review or completion boundary")
    return completion


def run(qc_root, aniflow, output):
    base.require(not Path(qc_root).is_symlink(), "canonical prior QC directory required")
    qc_root = Path(qc_root).resolve(strict=True)
    output = qc.safe_new_directory(output, qc.ROOT)
    base.require(qc_root not in output.parents and output not in qc_root.parents,
                 "metadata output must be disjoint from prior QC evidence")
    previous = base.read_json(qc_root / "qc-pilot-report.json")
    base.require(previous.get("schema") == "aniflow.synthetic-final-qc-pilot/v1"
                 and previous.get("state") == "passed", "a passed exact-final QC pilot is required")
    prior_snapshot = qc.snapshot(qc_root)
    pilot = qc.Pilot(Path(previous["baseline_directory"]), aniflow, output)
    pilot.report.update(schema="aniflow.synthetic-metadata-qc-pilot/v1",
                        transport_harness=pilot.report["harness"], harness=base.file_record(Path(__file__)))
    try:
        old_qc_root = Path(previous["qc_directory"]).resolve(strict=True)
        base.require(qc_root in old_qc_root.parents and previous["baseline"] == pilot.initial,
                     "prior QC escaped its original exact pilot binding")
        old_pin = previous["qc_receipt_sha256"]
        old_qc = pilot.command("original-qc-before", "delivery_audio_qc_status", ["delivery", "audio-qc", "status",
            "--directory", old_qc_root, "--receipt-sha256", old_pin])
        original = pilot.base_report["outputs"]["h264_delivery"]
        old_measurement = qc.observe_qc(old_qc, original["sha256"], original["bytes"])
        old_provider = Path(old_qc["intent"]["request"]["provider_executable"]).resolve(strict=True)
        base.require(base.file_record(old_provider, base.MAX_TOOL) == previous["tools"]["aniflow"], "original QC provider changed")
        pilot.tool_records[old_provider] = previous["tools"]["aniflow"]
        configuration = copy.deepcopy(pilot.configuration)
        configuration["delivery_metadata"] = {"schema": "aniflow.delivery-metadata-policy/v1",
            "mode": "clear_inherited_then_apply_allowlist", "approved": {"title": TITLE}}
        request = {"schema": "aniflow.finishing-request/v1", "video_run_directory": str(pilot.owner),
            "artifact_id": "h264_delivery", "provider_executable": str(pilot.aniflow), "captions": None,
            "metadata": configuration}
        path = pilot.write("finishing-request.json", request)
        plan = pilot.command("finishing-plan", "video_finishing_plan", ["video", "finishing", "plan", "--request", path])
        base.require(plan["intent"]["payload"]["request"] == request
                     and len(plan["steps"]) == 1 and plan["steps"][0]["phase"] == "metadata_remux",
                     "finishing plan changed the selected metadata-only request")
        finishing = pilot.output / "finishing"
        value = pilot.command("finishing-run", "video_finishing_run", ["video", "finishing", "run", "--request", path,
                                                                        "--run-directory", finishing])
        completion = observe_finishing(value)
        artifact = completion["artifacts"][0]
        candidate = base.contained_file(artifact["locator"], pilot.output)
        candidate_record = base.file_record(candidate)
        base.require(candidate_record["sha256"] == artifact["sha256"] and candidate_record["bytes"] == artifact["byte_count"]
                     and candidate_record["sha256"] != original["sha256"], "metadata did not produce a distinct exact final file")

        def tool(name, argv):
            step = execute(argv, cwd=pilot.output, logs=pilot.output / "logs", name=name, deadline=pilot.deadline,
                           environment=dict(os.environ), budget_root=pilot.output,
                           maximum_bytes=qc.MAXIMUM - qc.REPORT_MAXIMUM)
            pilot.report["steps"].append(step)
            require_passed(step)

        def probe(name, path):
            tool(name, [pilot.tools["ffprobe"], "-v", "error", "-count_frames", "-show_streams", "-show_format",
                        "-show_packets", "-show_data_hash", "sha256", "-of", "json", path])
            return base.read_json(pilot.output / "logs" / (name + ".stdout"))

        source = base.contained_file(str(pilot.pilot_root / original["path"]), pilot.pilot_root)
        before, after = probe("probe-original", source), probe("probe-metadata", candidate)
        _, before_audio = base.media_summary(before, False)
        media, after_audio = base.media_summary(after, False)
        video = video_identity(after)
        base.require(video_identity(before) == video and before_audio == after_audio,
                     "metadata remux changed elementary packet/configuration/clock/priming identities")
        base.require(after["format"]["tags"].get("title") == TITLE, "selected synthetic title was not observed")
        tool("decode-metadata", [pilot.tools["ffmpeg"], "-hide_banner", "-nostdin", "-v", "error", "-xerror",
                                 "-i", candidate, "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
        status = pilot.command("finishing-status", "video_finishing_status", ["video", "finishing", "status", "--run-directory", finishing])
        base.require(observe_finishing(status) == completion, "finishing readback changed completion")
        resumed = pilot.command("finishing-resume", "video_finishing_resume", ["video", "finishing", "resume", "--run-directory", finishing])
        base.require(observe_finishing(resumed) == completion, "completed finishing resume changed completion")
        projected = pilot.command("completion", "video_completion", ["video", "completion", finishing])
        base.require(projected == completion, "final-video projection disagrees with finishing")
        action = pilot.command("next-action", "video_finishing_next_action", ["video", "finishing", "next-action", "--run-directory", finishing])
        base.require(action["action"] == {"kind": "review_final_candidate"}
                     and action["execution_authority"] == "none; explicitly invoke the existing run/resume/review owner and its native guards"
                     and not (finishing / "caption-review.json").exists(), "metadata-only finishing invented approval or caption work")
        request = {"schema": "aniflow.final-audio-qc-request/v1", "source_run_directory": str(finishing),
            "artifact_id": "metadata_delivery", "configuration": configuration,
            "provider_executable": str(pilot.aniflow), "maximum_pcm_bytes": 2097152}
        path = pilot.write("new-qc-request.json", request)
        qc_plan = pilot.command("new-qc-plan", "delivery_audio_qc_plan", ["delivery", "audio-qc", "plan", "--request", path])
        base.require(qc_plan["request"] == request, "new QC plan changed its selected request")
        new_qc_root = pilot.output / "new-qc"
        new_qc = pilot.command("new-qc-run", "delivery_audio_qc_run", ["delivery", "audio-qc", "run", "--request", path,
            "--output-directory", new_qc_root])
        base.require(new_qc["intent"] == qc_plan and base.read_json(new_qc_root / "qc.json") == new_qc, "new QC marker/intent changed")
        measurement = qc.observe_qc(new_qc, candidate_record["sha256"], candidate_record["bytes"])
        base.require(measurement["decoded_pcm_sha256"] == old_measurement["decoded_pcm_sha256"], "metadata remux changed decoded PCM")
        new_pin = base.file_record(new_qc_root / "qc.json")["sha256"]
        new_snapshot = qc.snapshot(new_qc_root)
        current = pilot.command("new-qc-status", "delivery_audio_qc_status", ["delivery", "audio-qc", "status",
            "--directory", new_qc_root, "--receipt-sha256", new_pin])
        base.require(current == new_qc, "new QC status differs")
        profile = qc.synthetic_preflight(finishing, "metadata_delivery", new_qc_root, new_pin)
        path = pilot.write("matching-preflight.json", profile)
        match = pilot.command("matching-preflight", "delivery_preflight", ["delivery", "preflight", "--request", path])
        checks = qc.observe_preflight(match, profile, "matches_supplied_profile", "loudness_evidence", "matches")
        stale = copy.deepcopy(profile)
        stale["input"].update(qc_directory=str(old_qc_root), qc_receipt_sha256=old_pin)
        path = pilot.write("stale-preflight.json", stale)
        pilot.command("refuse-stale-qc", "delivery_preflight", ["delivery", "preflight", "--request", path],
                      refusal="final-file QC must bind the exact selected producer/artifact")
        unchanged = pilot.command("original-qc-after", "delivery_audio_qc_status", ["delivery", "audio-qc", "status",
            "--directory", old_qc_root, "--receipt-sha256", old_pin])
        base.require(unchanged == old_qc and qc.snapshot(qc_root) == prior_snapshot
                     and qc.snapshot(new_qc_root) == new_snapshot, "readback/refusal changed retained QC evidence")
        base.require_preserved_files({candidate: candidate_record}, "metadata candidate changed during resume/QC/refusal")
        pilot.preserve()
        pilot.report.update(state="passed", original_qc_inventory=prior_snapshot, original_qc_preserved=True,
            metadata_file={**candidate_record, "media": media, "synthetic_title": TITLE}, measurement=measurement,
            original_final_sha256=original["sha256"], source_bytes_changed=True, video_packets_preserved=64,
            aac_packets_preserved=len(after_audio["packets"]),
            video_packet_identity_sha256=hashlib.sha256(base.json_bytes(video)).hexdigest(),
            new_qc_receipt_sha256=new_pin, stale_qc_refused=True, finishing_resume_preserved=True,
            completion_preserved=True, next_action="review_final_candidate", preflight_checks=checks,
            original_inventory_preserved=True)
    except (ValueError, OSError, KeyError, TypeError, ZeroDivisionError) as error:
        pilot.report["failure"] = str(error)[:2000]
    finally:
        try:
            base.require(qc.snapshot(qc_root) == prior_snapshot, "prior QC inventory changed")
        except (ValueError, OSError) as error:
            pilot.report.update(state="failed", failure=str(error)[:2000])
        pilot.finish()
    return pilot.report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qc-pilot-root", type=Path, required=True)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        report = run(args.qc_pilot_root, args.aniflow, args.output)
    except (ValueError, OSError) as error:
        print(str(error), file=sys.stderr)
        return 2
    print(json.dumps({"state": report["state"], "report": str(args.output / "qc-pilot-report.json"), "release_qualified": False}))
    return 0 if report["state"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
