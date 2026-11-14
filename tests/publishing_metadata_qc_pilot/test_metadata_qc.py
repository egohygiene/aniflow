"""Independent packet identity and no-review finishing observations."""
import copy
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("metadata_qc", ROOT / "scripts/publishing-metadata-qc-pilot.py")
pilot = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pilot)


def document():
    return {"streams": [{"index": 0, "codec_type": "video", "codec_name": "h264", "profile": "High",
        "level": 11, "extradata_size": 40, "extradata_hash": "SHA256:" + "a" * 64,
        "is_avc": "true", "nal_length_size": "4", "time_base": "1/24"}],
        "packets": [{"stream_index": 0, "pts": i, "dts": i, "duration": 1,
            "size": "100", "data_hash": "SHA256:" + "b" * 64, "flags": "K_"} for i in range(64)]}


def finishing():
    return {"schema": "aniflow.finishing-status/v1", "complete": True, "preview": None, "review": None,
        "publication_authority": "none", "creative_approval":
        "full_delivery_review_not_performed; preview_decision_only_authorizes_candidate_generation",
        "steps": [{"phase": "metadata_remux", "state": "complete"}], "completion": {
            "schema": "aniflow.final-video-handoff/v1", "creative_approval": "not_recorded_by_fixed_automated_profile",
            "publication_authority": "none", "proof_state": "not_asserted",
            "artifacts": [{"artifact_id": "metadata_delivery", "variant": "destination_copy"}]}}


class MetadataQcObservations(unittest.TestCase):
    def test_output_inside_prior_qc_refuses_before_creating_or_reading_work(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "canary").write_bytes(b"retain")
            original = pilot.qc.snapshot(root)
            with self.assertRaises(ValueError):
                pilot.run(root, Path("/missing/tool"), root / "nested-output")
            self.assertEqual(pilot.qc.snapshot(root), original)

    def test_rational_packet_clock_rescaling_is_equal_but_timing_or_payload_changes_are_not(self):
        source = document()
        before = pilot.video_identity(source)
        rescaled = copy.deepcopy(source)
        rescaled["streams"][0]["time_base"] = "1/48"
        for packet in rescaled["packets"]:
            for key in ["pts", "dts", "duration"]:
                packet[key] *= 2
        self.assertEqual(pilot.video_identity(rescaled), before)
        for key, value in [("pts", 1), ("dts", 1), ("duration", 2), ("data_hash", "SHA256:" + "c" * 64)]:
            changed = copy.deepcopy(source)
            changed["packets"][0][key] = value
            with self.subTest(key=key):
                self.assertNotEqual(pilot.video_identity(changed), before)
        changed = copy.deepcopy(source)
        changed["streams"][0]["extradata_hash"] = "SHA256:" + "c" * 64
        self.assertNotEqual(pilot.video_identity(changed), before)

    def test_missing_packet_or_digest_cannot_establish_complete_stream_copy(self):
        for kind in ["missing", "digest"]:
            changed = document()
            if kind == "missing":
                changed["packets"].pop()
            else:
                changed["packets"][0]["data_hash"] = "not-a-digest"
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                pilot.video_identity(changed)

    def test_metadata_completion_cannot_invent_caption_work_review_or_publication(self):
        pilot.observe_finishing(finishing())
        for key, value in [("review", {"decision": "approved"}), ("preview", {}),
                            ("complete", False), ("publication_authority", "posted"), ("completion", None)]:
            changed = finishing()
            changed[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                pilot.observe_finishing(changed)
        changed = finishing()
        changed["completion"]["artifacts"][0]["artifact_id"] = "original_delivery"
        with self.assertRaises(ValueError):
            pilot.observe_finishing(changed)


if __name__ == "__main__":
    unittest.main()
