"""Negative exact-subject observations; no product or media commands are launched."""
import copy
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("publishing_qc", ROOT / "scripts/publishing-qc-pilot.py")
qc = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qc)


def quantity(value=-20.0, unit="loudness_units_full_scale"):
    return {"unit": unit, "value": {"kind": "measured", "value": value}}


def report():
    return {"schema": "aniflow.final-audio-qc/v1", "normalization": "none", "runtime_qualified": False,
        "creative_approval": "not_performed; measurements are not musical quality review", "publication_authority": "none",
        "intent": {"artifact": {"sha256": "a" * 64, "byte_count": 107194}},
        "accepted_pcm": {"source": {"frame_count": 128000, "sample_rate_hz": 48000, "channels": 2,
                                      "artifact": {"sha256": "b" * 64}}},
        "signal": {"source_sha256": "a" * 64, "frames": 128000, "sample_rate_hz": 48000, "channels": 2,
            "source_bytes": 107194, "source_audio_extent": {"start": {"numerator": 0, "denominator": 1},
                                                            "end": {"numerator": 8, "denominator": 3}},
            "decoded_wav_sha256": "b" * 64, "decoded_pcm_sha256": "c" * 64, "normalization": "none",
            "runtime_qualified": False, "loudness": {"frames": 128000, "sample_rate_hz": 48000, "channels": 2,
                "pcm_sha256": "c" * 64, "runtime_qualified": False, "integrated_loudness": quantity(),
                "true_peak": quantity(-1.5, "decibels_true_peak")}}}


class PublishingQcObservations(unittest.TestCase):
    def test_valid_looking_error_cannot_hide_a_panic_exit(self):
        error = {"category": "state", "message": "selected refusal"}
        qc.require_state_refusal({"state": "failed", "exit_code": 8}, error, "selected refusal")
        for code in [None, 0, 1, 2, 7, 101]:
            with self.subTest(code=code), self.assertRaises(ValueError):
                qc.require_state_refusal({"state": "failed", "exit_code": code}, error, "selected refusal")

    def test_unavailable_nonfinite_wrong_unit_and_boolean_are_not_measurements(self):
        self.assertEqual(qc.finite_measurement(quantity(), "loudness_units_full_scale"), -20)
        for value in [float("nan"), float("inf"), True, None, "-20"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                qc.finite_measurement(quantity(value), "loudness_units_full_scale")
        for bad in [{"unit": "loudness_units_full_scale", "value": {"kind": "unavailable", "reason": "not measured"}},
                    quantity(0, "decibels_true_peak")]:
            with self.assertRaises(ValueError):
                qc.finite_measurement(bad, "loudness_units_full_scale")

    def test_qc_requires_exact_final_subject_and_complete_stereo_pcm(self):
        qc.observe_qc(report(), "a" * 64, 107194)
        for field, value in [("source_sha256", "d" * 64), ("frames", 127999), ("channels", 1),
                             ("decoded_pcm_sha256", "d" * 64), ("decoded_wav_sha256", "d" * 64)]:
            bad = report()
            bad["signal"][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                qc.observe_qc(bad, "a" * 64, 107194)
        bad = report()
        bad["runtime_qualified"] = True
        with self.assertRaises(ValueError):
            qc.observe_qc(bad, "a" * 64, 107194)

    def test_successful_preflight_needs_the_selected_state_and_retains_authority_limits(self):
        value = {"schema": "aniflow.destination-preflight/v1", "payload": {
            "request": {"selected_owner": "fixture"},
            "disposition": "missing_evidence", "creative_approval": "not_performed", "publication_authority": "none",
            "requirement_authority": "operator_supplied_dated_requirements; not independently verified live platform policy",
            "checks": [{"id": "loudness_evidence", "state": "missing_evidence"}]}}
        qc.observe_preflight(value, {"selected_owner": "fixture"}, "missing_evidence", "loudness_evidence", "missing_evidence")
        for key, change in [("disposition", "matches_supplied_profile"), ("creative_approval", "approved"),
                            ("publication_authority", "posted")]:
            bad = copy.deepcopy(value)
            bad["payload"][key] = change
            with self.subTest(key=key), self.assertRaises(ValueError):
                qc.observe_preflight(bad, {"selected_owner": "fixture"}, "missing_evidence", "loudness_evidence", "missing_evidence")
        with self.assertRaises(ValueError):
            qc.observe_preflight(value, {"selected_owner": "other"}, "missing_evidence", "loudness_evidence", "missing_evidence")

    def test_synthetic_comparison_dates_and_requirements_are_explicit(self):
        value = qc.synthetic_preflight(Path("/fixture/owner"), "h264_delivery", Path("/fixture/qc"), "a" * 64)
        self.assertEqual(value["input"]["kind"], "accepted_video_with_qc")
        self.assertTrue(value["profile"]["require_loudness_evidence"])
        self.assertEqual(value["profile"]["requirements"]["required_width"], 160)
        self.assertLessEqual(value["profile"]["evidence"]["checked_on"], value["evaluated_on"])
        self.assertLessEqual(value["evaluated_on"], value["profile"]["evidence"]["valid_through"])
        self.assertIn("not platform requirements", value["profile"]["evidence"]["checked_by"])

    def test_complete_inventory_detects_master_and_lyrics_changes_and_symbolic_members(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            master, words = root / "master.mp4", root / "manual.srt"
            master.write_bytes(b"master")
            words.write_bytes(b"exact supplied words")
            original = qc.snapshot(root)
            (root / "empty").mkdir()
            self.assertNotEqual(qc.snapshot(root), original)
            (root / "empty").rmdir()
            master.write_bytes(b"changed")
            self.assertNotEqual(qc.snapshot(root), original)
            master.write_bytes(b"master")
            words.write_bytes(b"changed words")
            self.assertNotEqual(qc.snapshot(root), original)
            (root / "link").symlink_to(master)
            with self.assertRaises(ValueError):
                qc.snapshot(root)

    def test_existing_output_refuses_before_selected_tool_or_input_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            canary = root / "canary"
            canary.write_bytes(b"retain")
            with self.assertRaises(ValueError):
                qc.Pilot(root, Path("/missing/tool"), root)
            self.assertEqual(canary.read_bytes(), b"retain")


if __name__ == "__main__":
    unittest.main()
