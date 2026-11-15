"""Adversarial saved-file observations; no product/media process is launched."""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("window_observer", ROOT / "scripts/presented-window-observer.py")
observer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(observer)
CAPTURE = None


class PresentedWindowObservations(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if CAPTURE is None:
            raise unittest.SkipTest("supply --capture-root for the retained native observation")
        cls.qc = json.loads((CAPTURE / "direct/qc.json").read_bytes())
        cls.signal = cls.qc["signal"]
        cls.source = Path(cls.qc["intent"]["artifact"]["locator"]).read_bytes()
        cls.complete = (CAPTURE / "direct/signal/selected-audio.wav").read_bytes()
        cls.presented = (CAPTURE / "direct/signal/presented-window/selected-audio.wav").read_bytes()

    def observe(self, signal=None, qc=None, source=None, complete=None, presented=None):
        return observer.observe_bytes(self.source if source is None else source,
            self.complete if complete is None else complete,
            self.presented if presented is None else presented,
            self.signal if signal is None else signal, qc)

    def test_actual_separate_subjects_and_explicit_request_join(self):
        result = self.observe(qc=self.qc)
        self.assertTrue(result["request_selection_checked"])
        self.assertTrue(result["exact_sample_byte_identity"])
        self.assertEqual(result["complete_pcm"]["decoded_sample_frames"], 96256)
        self.assertEqual(result["presented_pcm"]["decoded_sample_frames"], 96000)
        self.assertEqual(result["complete_pcm"]["nonpresented_tail_nonzero_values"], 512)
        self.assertFalse(result["native_meter_qualified"])
        self.assertFalse(result["publication_authority"])

    def test_changed_source_bytes_cannot_reuse_window(self):
        source = bytearray(self.source)
        source[-1] ^= 1
        with self.assertRaises(ValueError):
            self.observe(source=source)

    def test_repinning_wrong_view_bytes_cannot_bypass_source_comparison(self):
        data = bytearray(self.presented)
        data[56] ^= 1
        signal = copy.deepcopy(self.signal)
        window = signal["presented_window"]
        window["window_wav_sha256"] = hashlib.sha256(data).hexdigest()
        window["window_pcm_sha256"] = hashlib.sha256(data[56:]).hexdigest()
        window["loudness"]["pcm_sha256"] = window["window_pcm_sha256"]
        with self.assertRaisesRegex(ValueError, "exact canonical"):
            self.observe(signal=signal, presented=data)

    def test_nonpresented_complete_tail_stays_part_of_full_subject(self):
        data = bytearray(self.complete)
        offset = len(data) - 96256 * 8
        data[offset + 96000 * 8] ^= 1
        with self.assertRaises(ValueError):
            self.observe(complete=data)

    def test_none_request_cannot_adopt_injected_window(self):
        qc = copy.deepcopy(self.qc)
        qc["intent"]["request"].pop("presented_window")
        qc["intent"]["signal_request"].pop("presented_window")
        with self.assertRaisesRegex(ValueError, "explicitly select"):
            self.observe(qc=qc)

    def test_wrong_scope_range_or_sample_origin_refuses(self):
        for field, value in [("metric_scope", "complete_decoded_pcm_including_nonpresented_tail"),
                             ("start_sample_frame", 1),
                             ("source_audio_extent", observer.tail.sample_extent(95999))]:
            signal = copy.deepcopy(self.signal)
            signal["presented_window"][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.observe(signal=signal)

    def test_unknown_window_fields_cannot_imply_approval(self):
        signal = copy.deepcopy(self.signal)
        signal["presented_window"]["approved"] = True
        with self.assertRaises(ValueError):
            self.observe(signal=signal)

    def test_counts_keep_strict_json_integer_types(self):
        for value in [True, 96256.0, "96256"]:
            signal = copy.deepcopy(self.signal)
            signal["presented_window"]["complete_sample_frames"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.observe(signal=signal)

    def test_full_and_window_meters_cannot_swap_sample_scope(self):
        for selected, frames in [(False, 96000), (True, 96256)]:
            signal = copy.deepcopy(self.signal)
            meter = signal["presented_window"]["loudness"] if selected else signal["loudness"]
            meter["frames"] = frames
            with self.subTest(selected=selected), self.assertRaises(ValueError):
                self.observe(signal=signal)

    def test_missing_window_is_not_inferred_from_complete_meter(self):
        signal = copy.deepcopy(self.signal)
        signal.pop("presented_window")
        with self.assertRaises(ValueError):
            self.observe(signal=signal)

    def test_temporal_pin_is_sha256_and_must_match_both_receipts(self):
        for value, both in [("f" * 64, False), ("z" * 64, True)]:
            signal = copy.deepcopy(self.signal)
            signal["presented_window"]["temporal_sha256"] = value
            if both:
                signal["temporal_sha256"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.observe(signal=signal)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture-root", required=True, type=Path)
    args, rest = parser.parse_known_args()
    CAPTURE = args.capture_root.resolve(strict=True)
    unittest.main(argv=[__file__, *rest])
