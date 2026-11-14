#!/usr/bin/env python3
"""Adversarial readback of selected retained native audio captures; no processes."""
import argparse
import copy
import importlib.util
from pathlib import Path
import struct
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("tail_observer", ROOT / "scripts/publishing-audio-tail-observer.py")
observer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(observer)
CAPTURE_ROOT = None


class RetainedAudioObserverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if CAPTURE_ROOT is None:
            raise unittest.SkipTest("supply --capture-root for retained native observations")
        cls.padded = observer.strict_json((CAPTURE_ROOT / "fixtures/padded-audio.json").read_bytes())
        cls.ordinary = observer.strict_json((CAPTURE_ROOT / "fixtures/ordinary-audio.json").read_bytes())
        cls.aligned = observer.strict_json((CAPTURE_ROOT / "fixtures/cropped-audio.json").read_bytes())
        waves = list((CAPTURE_ROOT / "baseline-opt-padded").rglob("selected-audio.wav"))
        if len(waves) != 1:
            raise ValueError("expected one retained baseline padded WAV")
        cls.wav = waves[0].read_bytes()

    def native(self):
        return observer.native_audio(copy.deepcopy(self.padded))

    def signal(self):
        native = self.native()
        pcm = observer.float32_wav(self.wav, native)
        # This fixture is an observer unit-test input, never a product receipt.
        signal = {"schema": "aniflow.video-audio-signal/v1", "normalization": "none",
            "runtime_qualified": False, "source_audio_extent": observer.sample_extent(96000),
            "audio_stream": 1, "sample_rate_hz": 48000, "channels": 2, "frames": 96256,
            "decoded_pcm_sha256": pcm["decoded_pcm_sha256"],
            "decoded_wav_sha256": pcm["wav_sha256"], "decoded_wav_bytes": pcm["wav_bytes"],
            "decoded_audio_tail": {"schema": "aniflow.aac-decoded-tail/v1",
                "metric_scope": observer.DECODED_SCOPE, "presented_sample_frames": 96000,
                "decoded_sample_frames": 96256, "nonpresented_tail_sample_frames": 256,
                "decoded_block_count": 94, "final_presented_sample_frames": 768,
                "priming_sample_frames": 1024, "decoded_extent": observer.sample_extent(96256)},
            "loudness": {"frames": 96256, "sample_rate_hz": 48000, "channels": 2,
                "pcm_sha256": pcm["decoded_pcm_sha256"], "runtime_qualified": False}}
        return native, pcm, signal

    def changed_event(self, event_type, position=0):
        value = copy.deepcopy(self.padded)
        selected = [event for event in value["packets_and_frames"] if event["type"] == event_type]
        return value, selected[position]

    def test_actual_padded_and_ordinary_counts_are_exact(self):
        padded = self.native()
        ordinary = observer.native_audio(self.ordinary)
        for actual in (padded, ordinary):
            self.assertEqual((actual["presented_sample_frames"], actual["decoded_sample_frames"],
                actual["nonpresented_tail_sample_frames"], actual["decoded_block_count"],
                actual["packet_count"], actual["priming_sample_frames"],
                actual["final_presented_sample_frames"]), (96000, 96256, 256, 94, 95, 1024, 768))
            self.assertEqual(actual["required_meter_scope"], observer.DECODED_SCOPE)
        self.assertEqual(padded["aac_packet_identity_sha256"], ordinary["aac_packet_identity_sha256"])

    def test_actual_aligned_subset_has_no_tail(self):
        value = observer.native_audio(self.aligned)
        self.assertEqual((value["presented_sample_frames"], value["decoded_sample_frames"],
            value["nonpresented_tail_sample_frames"], value["decoded_block_count"],
            value["packet_count"]), (128000, 128000, 0, 125, 126))
        self.assertEqual(value["required_meter_scope"], observer.PRESENTED_SCOPE)

    def test_actual_complete_pcm_preserves_nonzero_tail(self):
        value = observer.float32_wav(self.wav, self.native())
        self.assertEqual(value["decoded_sample_frames"], 96256)
        self.assertEqual(value["decoded_pcm_sha256"],
            "b70f04f7cd3c64fbb6c80784e69f5d10ef29c5df3643695dfe04be47d0c2a4ee")
        self.assertEqual(value["nonpresented_tail_sha256"],
            "319f649cf63dfe3cac8116187d67cd432fd3f8459f77d6e299f80b45097b564b")
        self.assertEqual(value["nonpresented_tail_nonzero_values"], 512)
        self.assertGreater(value["nonpresented_tail_peak_absolute"], 0)

    def test_interior_short_frame_refuses(self):
        value, frame = self.changed_event("frame", 1)
        frame["duration"] = 768
        with self.assertRaises(ValueError):
            observer.native_audio(value)

    def test_gap_overlap_or_changed_decode_clock_refuses(self):
        for delta in (-1, 1):
            value, frame = self.changed_event("frame", 2)
            frame["pts"] += delta
            with self.subTest(delta=delta), self.assertRaises(ValueError):
                observer.native_audio(value)
        value, frame = self.changed_event("frame", 2)
        frame["pkt_dts"] += 1
        with self.assertRaises(ValueError):
            observer.native_audio(value)

    def test_larger_or_missing_decoded_block_refuses(self):
        for replacement in (0, 768, 2048):
            value, frame = self.changed_event("frame", -1)
            frame["nb_samples"] = replacement
            with self.subTest(samples=replacement), self.assertRaises(ValueError):
                observer.native_audio(value)

    def test_priming_skip_discard_or_reason_change_refuses(self):
        for field, replacement in (("skip_samples", 0), ("discard_padding", 256),
                                   ("skip_reason", 1), ("discard_reason", 1)):
            value, packet = self.changed_event("packet")
            packet["side_data_list"][0][field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                observer.native_audio(value)
        value, packet = self.changed_event("packet")
        packet["side_data_list"] = []
        with self.assertRaises(ValueError):
            observer.native_audio(value)

    def test_late_discard_padding_refuses(self):
        value, packet = self.changed_event("packet", -1)
        packet["side_data_list"] = [{"side_data_type": "Skip Samples", "skip_samples": 0,
            "discard_padding": 256, "skip_reason": 0, "discard_reason": 0}]
        with self.assertRaises(ValueError):
            observer.native_audio(value)

    def test_packet_duration_and_stream_extent_mismatch_refuse(self):
        value, packet = self.changed_event("packet", -1)
        packet["duration"] = 1024
        with self.assertRaises(ValueError):
            observer.native_audio(value)
        value = copy.deepcopy(self.padded)
        value["streams"][0]["duration_ts"] = 96256
        with self.assertRaises(ValueError):
            observer.native_audio(value)

    def test_extra_or_wrong_stream_observations_refuse(self):
        value = copy.deepcopy(self.padded)
        value["streams"].append(copy.deepcopy(value["streams"][0]))
        with self.assertRaises(ValueError):
            observer.native_audio(value)
        value, frame = self.changed_event("frame")
        frame["stream_index"] = 0
        with self.assertRaises(ValueError):
            observer.native_audio(value)

    def test_pcm_count_cannot_be_relabelled_presented_extent(self):
        native = self.native()
        native["decoded_sample_frames"] = native["presented_sample_frames"]
        with self.assertRaises(ValueError):
            observer.float32_wav(self.wav, native)

    def test_nonfinite_pcm_and_changed_riff_fact_refuse(self):
        value = bytearray(self.wav)
        offset = value.index(b"data") + 8
        struct.pack_into("<f", value, offset, float("nan"))
        with self.assertRaises(ValueError):
            observer.float32_wav(value, self.native())
        value = bytearray(self.wav)
        offset = value.index(b"fact") + 8
        struct.pack_into("<I", value, offset, 96000)
        with self.assertRaises(ValueError):
            observer.float32_wav(value, self.native())

    def test_packet_or_configuration_changes_are_not_identity_preserving(self):
        original = self.native()["aac_packet_identity_sha256"]
        value, packet = self.changed_event("packet", 2)
        packet["data_hash"] = "SHA256:" + "0" * 64
        self.assertNotEqual(observer.native_audio(value)["aac_packet_identity_sha256"], original)
        value = copy.deepcopy(self.padded)
        value["streams"][0]["extradata_hash"] = "SHA256:" + "0" * 64
        self.assertNotEqual(observer.native_audio(value)["aac_packet_identity_sha256"], original)

    def test_duplicate_or_nonfinite_json_refuses(self):
        for data in (b'{"a":1,"a":2}', b'{"a":NaN}', b'{"a":Infinity}'):
            with self.subTest(data=data), self.assertRaises(ValueError):
                observer.strict_json(data)

    def test_truthful_unit_receipt_joins_full_pcm_and_keeps_presented_unverified(self):
        native, pcm, signal = self.signal()
        joined = observer.bind_signal(native, pcm, signal)
        self.assertEqual(joined["metric_scope"], observer.DECODED_SCOPE)
        self.assertIs(joined["exact_complete_pcm_join"], True)
        self.assertIs(joined["presented_delivery_loudness_verified"], False)

    def test_receipt_missing_extra_or_wrong_tail_fields_refuse(self):
        native, pcm, signal = self.signal()
        for change in ("absent", "null", "extra", "wrong_count", "wrong_scope", "wrong_extent"):
            changed = copy.deepcopy(signal)
            if change == "absent":
                del changed["decoded_audio_tail"]
            elif change == "null":
                changed["decoded_audio_tail"] = None
            elif change == "extra":
                changed["decoded_audio_tail"]["presented_delivery_loudness_verified"] = True
            elif change == "wrong_count":
                changed["decoded_audio_tail"]["nonpresented_tail_sample_frames"] = 0
            elif change == "wrong_scope":
                changed["decoded_audio_tail"]["metric_scope"] = observer.PRESENTED_SCOPE
            else:
                changed["decoded_audio_tail"]["decoded_extent"] = observer.sample_extent(96000)
            with self.subTest(change=change), self.assertRaises(ValueError):
                observer.bind_signal(native, pcm, changed)

    def test_pcm_hash_mismatch_and_presented_meter_count_refuse(self):
        native, pcm, signal = self.signal()
        for target, field, replacement in (("signal", "decoded_pcm_sha256", "0" * 64),
                                           ("meter", "pcm_sha256", "0" * 64),
                                           ("meter", "frames", 96000)):
            changed = copy.deepcopy(signal)
            (changed if target == "signal" else changed["loudness"])[field] = replacement
            with self.subTest(target=target, field=field), self.assertRaises(ValueError):
                observer.bind_signal(native, pcm, changed)

    def test_presented_extent_cannot_be_replaced_by_decoded_duration(self):
        native, pcm, signal = self.signal()
        signal["source_audio_extent"] = observer.sample_extent(96256)
        with self.assertRaises(ValueError):
            observer.bind_signal(native, pcm, signal)

    def test_receipt_boolean_or_string_numbers_refuse(self):
        native, pcm, signal = self.signal()
        for change in ("stream_boolean", "frame_string", "extent_boolean", "tail_float"):
            changed = copy.deepcopy(signal)
            if change == "stream_boolean":
                changed["audio_stream"] = True
            elif change == "frame_string":
                changed["frames"] = "96256"
            elif change == "extent_boolean":
                changed["source_audio_extent"]["start"]["numerator"] = False
            else:
                changed["decoded_audio_tail"]["decoded_block_count"] = 94.0
            with self.subTest(change=change), self.assertRaises(ValueError):
                observer.bind_signal(native, pcm, changed)

    def test_aligned_unit_receipt_requires_absent_extension(self):
        native = observer.native_audio(self.aligned)
        _, pcm, signal = self.signal()
        # No aligned WAV was produced by this observer. These are only joined
        # unit-test facts, used to check preservation of the old absent field.
        pcm = dict(pcm, decoded_sample_frames=128000)
        signal.update(frames=128000, source_audio_extent=observer.sample_extent(128000))
        signal["loudness"]["frames"] = 128000
        del signal["decoded_audio_tail"]
        observer.bind_signal(native, pcm, signal)
        signal["decoded_audio_tail"] = None
        with self.assertRaises(ValueError):
            observer.bind_signal(native, pcm, signal)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture-root", required=True, type=Path)
    args, remaining = parser.parse_known_args()
    CAPTURE_ROOT = args.capture_root.resolve(strict=True)
    unittest.main(argv=[__file__] + remaining)
