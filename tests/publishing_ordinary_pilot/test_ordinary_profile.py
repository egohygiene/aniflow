"""Profile-selection observations only; no product or media command is launched."""
import copy
from fractions import Fraction
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


base = load("ordinary_profile_base", "scripts/publishing-pilot.py")
qc = load("ordinary_profile_qc", "scripts/publishing-qc-pilot.py")
metadata = load("ordinary_profile_metadata", "scripts/publishing-metadata-qc-pilot.py")
base_fixture = load("ordinary_base_fixture", "tests/publishing_pilot/test_publishing_pilot.py")
qc_fixture = load("ordinary_qc_fixture", "tests/publishing_qc_pilot/test_publishing_qc_pilot.py")


def ordinary_probe():
    document = base_fixture.probe()
    document["streams"][0].update(height=90, nb_read_frames="48", duration_ts=24576)
    document["streams"][1]["duration_ts"] = 96000
    return document


def ordinary_qc():
    value = qc_fixture.report()
    signal = value["signal"]
    signal["source_audio_extent"]["end"] = {"numerator": 2, "denominator": 1}
    signal["frames"] = value["accepted_pcm"]["source"]["frame_count"] = signal["loudness"]["frames"] = 96256
    signal["decoded_wav_bytes"] = value["accepted_pcm"]["source"]["artifact"]["byte_size"] = 770106
    signal["decoded_audio_tail"] = {"schema": "aniflow.aac-decoded-tail/v1",
        "metric_scope": "complete_decoded_pcm_including_nonpresented_tail",
        "presented_sample_frames": 96000, "decoded_sample_frames": 96256,
        "nonpresented_tail_sample_frames": 256, "decoded_block_count": 94,
        "final_presented_sample_frames": 768, "priming_sample_frames": 1024,
        "decoded_extent": {"start": {"numerator": 0, "denominator": 1},
                           "end": {"numerator": 752, "denominator": 375}}}
    return value


def metadata_probe(frames):
    document = ordinary_probe() if frames == 48 else base_fixture.probe()
    document["streams"][0].update(profile="High", level=11, extradata_size=44,
        extradata_hash="SHA256:" + "d" * 64, is_avc="true", nal_length_size="4")
    document["packets"].extend({"stream_index": 0, "pts": index * 512,
        "dts": index * 512, "duration": 512, "size": 10,
        "data_hash": "SHA256:" + "f" * 64, "flags": "K_" if index == 0 else "__"}
        for index in range(frames))
    return document


def extraction_evidence():
    return {"schema": "aniflow.frame-extraction-report/v1", "width": 160,
        "height": 90, "frame_count": 48, "source_sha256": "a" * 64,
        "source_byte_count": 82060,
        "media": {"profile": "aniflow.ffmpeg-8.0.1-rgb8-cfr-source-extraction/coded-display-v2",
            "width": 160, "height": 90, "frame_count": 48, "native_qualified": False,
            "source_geometry": {"coded_width": 160, "coded_height": 96,
                "display_width": 160, "display_height": 90,
                "crop_left": 0, "crop_right": 0, "crop_top": 0, "crop_bottom": 6}}}


class OrdinaryProfileObservations(unittest.TestCase):
    def test_selected_profiles_keep_distinct_complete_and_presented_counts(self):
        self.assertEqual(base.profile_facts("ordinary-mp4"), {"frames": 48, "height": 90,
            "samples": 96000, "decoded_samples": 96256, "duration": Fraction(2)})
        self.assertEqual(base.profile_facts(), {"frames": 64, "height": 96,
            "samples": 128000, "decoded_samples": 128000, "duration": Fraction(8, 3)})
        self.assertEqual(base.profile_facts(), base.profile_facts("aligned"))

    def test_unsupported_profile_refuses_pure_observers(self):
        for profile in ("ordinary", "auto", "", None):
            with self.subTest(profile=profile):
                with self.assertRaises(ValueError):
                    base.profile_facts(profile)
                with self.assertRaises(ValueError):
                    base.media_summary(ordinary_probe(), False, source_profile=profile)
                with self.assertRaises(ValueError):
                    qc.observe_qc(ordinary_qc(), "a" * 64, 107194, profile)
                with self.assertRaises(ValueError):
                    qc.synthetic_preflight(Path("owner"), "h264_delivery", Path("qc"), "a" * 64, profile)
                with self.assertRaises(ValueError):
                    base.observe_source_geometry(extraction_evidence(), profile, {"sha256": "a" * 64, "bytes": 82060})

    def test_ordinary_video_requires_48_frames_and_160_by_90_display(self):
        media, _ = base.media_summary(ordinary_probe(), False, source_profile="ordinary-mp4")
        self.assertEqual((media["frames"], media["width"], media["height"]), (48, 160, 90))
        for field, replacement in (("height", 96), ("width", 159), ("nb_read_frames", "47")):
            changed = ordinary_probe()
            changed["streams"][0][field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                base.media_summary(changed, False, source_profile="ordinary-mp4")

    def test_ordinary_declared_extent_stays_presented_96000_samples(self):
        media, _ = base.media_summary(ordinary_probe(), False, source_profile="ordinary-mp4")
        self.assertEqual((media["audio_duration_samples"], media["audio_duration"]), (96000, "2"))
        for samples in (95999, 96256):
            changed = ordinary_probe()
            changed["streams"][1]["duration_ts"] = samples
            with self.subTest(samples=samples), self.assertRaises(ValueError):
                base.media_summary(changed, False, source_profile="ordinary-mp4")

    def test_default_aligned_and_ordinary_captures_cannot_be_cross_selected(self):
        base.media_summary(base_fixture.probe(), False)
        qc.observe_qc(qc_fixture.report(), "a" * 64, 107194)
        for document, profile in ((ordinary_probe(), "aligned"), (base_fixture.probe(), "ordinary-mp4")):
            with self.subTest(profile=profile), self.assertRaises(ValueError):
                base.media_summary(document, False, source_profile=profile)
        with self.assertRaises(ValueError):
            qc.observe_qc(ordinary_qc(), "a" * 64, 107194)
        with self.assertRaises(ValueError):
            qc.observe_qc(qc_fixture.report(), "a" * 64, 107194, "ordinary-mp4")

    def test_ordinary_qc_keeps_full_decode_metrics_and_presented_goals_unverified(self):
        result = qc.observe_qc(ordinary_qc(), "a" * 64, 107194, "ordinary-mp4")
        self.assertEqual((result["frames"], result["presented_sample_frames"]), (96256, 96000))
        self.assertEqual(result["metric_scope"], "complete_decoded_pcm_including_nonpresented_tail")
        self.assertIs(result["presented_delivery_loudness_verified"], False)
        self.assertIs(result["native_meter_qualified"], False)
        self.assertEqual(result["normalization"], "none")

    def test_ordinary_qc_rejects_wrong_owner_counts_and_declared_extent(self):
        for owner in ("signal", "accepted_pcm", "meter", "presented_extent"):
            changed = ordinary_qc()
            if owner == "signal":
                changed["signal"]["frames"] = 96000
            elif owner == "accepted_pcm":
                changed["accepted_pcm"]["source"]["frame_count"] = 96000
            elif owner == "meter":
                changed["signal"]["loudness"]["frames"] = 96000
            else:
                changed["signal"]["source_audio_extent"]["end"] = {"numerator": 752, "denominator": 375}
            with self.subTest(owner=owner), self.assertRaises(ValueError):
                qc.observe_qc(changed, "a" * 64, 107194, "ordinary-mp4")

    def test_ordinary_qc_requires_exact_positive_integer_decoded_wav_byte_identity(self):
        qc.observe_qc(ordinary_qc(), "a" * 64, 107194, "ordinary-mp4")
        for owner, replacement in (("signal", 770105), ("accepted_pcm", 770105),
                                   ("signal", "770106"), ("accepted_pcm", 770106.0),
                                   ("signal", 0), ("accepted_pcm", True)):
            changed = ordinary_qc()
            if owner == "signal":
                changed["signal"]["decoded_wav_bytes"] = replacement
            else:
                changed["accepted_pcm"]["source"]["artifact"]["byte_size"] = replacement
            with self.subTest(owner=owner, replacement=replacement), self.assertRaises(ValueError):
                qc.observe_qc(changed, "a" * 64, 107194, "ordinary-mp4")

    def test_ordinary_qc_rejects_mislabeled_or_type_substituted_scope(self):
        for field, replacement in (("metric_scope", "complete_presented_audio"),
                                   ("decoded_block_count", 94.0)):
            changed = ordinary_qc()
            changed["signal"]["decoded_audio_tail"][field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                qc.observe_qc(changed, "a" * 64, 107194, "ordinary-mp4")
        changed = ordinary_qc()
        changed["signal"]["source_audio_extent"]["start"]["numerator"] = False
        with self.assertRaises(ValueError):
            qc.observe_qc(changed, "a" * 64, 107194, "ordinary-mp4")

    def test_synthetic_preflight_uses_profile_geometry_and_presented_clock(self):
        for profile, height, numerator, denominator in (("ordinary-mp4", 90, 2, 1), ("aligned", 96, 8, 3)):
            value = qc.synthetic_preflight(Path("owner"), "h264_delivery", Path("qc"), "a" * 64, profile)
            self.assertEqual(value["profile"]["requirements"]["required_height"], height)
            self.assertEqual(value["profile"]["maximum_duration"], {"numerator": numerator, "denominator": denominator})
            self.assertIs(value["profile"]["require_loudness_evidence"], True)
            self.assertIn("not platform requirements", value["profile"]["evidence"]["checked_by"])

    def test_source_configuration_selects_frame_bound_without_changing_owners(self):
        with tempfile.TemporaryDirectory() as temporary:
            tools = {}
            for name in ("ffmpeg", "ffprobe"):
                path = Path(temporary) / name
                path.write_bytes(("synthetic unit-test pin " + name).encode())
                tools[name] = path
            ordinary = base.source_configuration(tools, "ordinary-mp4")
            aligned = base.source_configuration(tools)
            self.assertEqual((ordinary["extraction"]["maximum_frames"], aligned["extraction"]["maximum_frames"]), (48, 64))
            ordinary["extraction"]["maximum_frames"] = 64
            self.assertEqual(ordinary, aligned)

    def test_metadata_packet_identity_observes_selected_complete_frame_count(self):
        actual = metadata.video_identity(metadata_probe(48), "ordinary-mp4")
        self.assertEqual(len(actual["packets"]), 48)
        self.assertEqual(len(metadata.video_identity(metadata_probe(64))["packets"]), 64)
        with self.assertRaises(ValueError):
            metadata.video_identity(metadata_probe(64), "ordinary-mp4")
        with self.assertRaises(ValueError):
            metadata.video_identity(metadata_probe(48))

    def test_ordinary_extraction_requires_exact_observed_coded_display_rectangle(self):
        source = {"sha256": "a" * 64, "bytes": 82060}
        base.observe_source_geometry(extraction_evidence(), "ordinary-mp4", source)
        for field, replacement in (("coded_height", 90), ("display_height", 96),
                                   ("crop_bottom", 0), ("crop_top", 2)):
            changed = extraction_evidence()
            changed["media"]["source_geometry"][field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                base.observe_source_geometry(changed, "ordinary-mp4", source)
        changed = extraction_evidence()
        del changed["media"]["source_geometry"]
        with self.assertRaises(ValueError):
            base.observe_source_geometry(changed, "ordinary-mp4", source)
        changed = extraction_evidence()
        changed["media"]["profile"] = "aniflow.ffmpeg-8.0.1-rgb8-cfr-source-extraction/v1"
        with self.assertRaises(ValueError):
            base.observe_source_geometry(changed, "ordinary-mp4", source)
        with self.assertRaises(ValueError):
            base.observe_source_geometry(extraction_evidence(), "ordinary-mp4", {"sha256": "b" * 64, "bytes": 82060})
        aligned = extraction_evidence()
        aligned.update(height=96, frame_count=64)
        aligned["media"].update(height=96, frame_count=64,
            profile="aniflow.ffmpeg-8.0.1-rgb8-cfr-source-extraction/v1")
        del aligned["media"]["source_geometry"]
        base.observe_source_geometry(aligned, "aligned", source)
        aligned["media"]["source_geometry"] = None
        with self.assertRaises(ValueError):
            base.observe_source_geometry(aligned, "aligned", source)


if __name__ == "__main__":
    unittest.main()
