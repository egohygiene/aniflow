"""Independent negative observations for the bounded typed CPU pilot."""

import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("publishing_pilot", ROOT / "scripts/publishing-pilot.py")
pilot = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pilot)


def probe(subtitles=False):
    streams = [
        {"index": 0, "codec_type": "video", "codec_name": "h264", "width": 160,
         "height": 96, "pix_fmt": "yuv420p", "nb_read_frames": "64", "avg_frame_rate": "24/1",
         "start_pts": 0, "duration_ts": 32768, "time_base": "1/12288"},
        {"index": 1, "codec_type": "audio", "codec_name": "aac", "sample_rate": "48000",
         "channels": 2, "channel_layout": "stereo", "time_base": "1/48000", "profile": "LC",
         "start_pts": 0, "duration_ts": 128000,
         "extradata_size": 5, "extradata_hash": "SHA256:" + "e" * 64},
    ]
    if subtitles:
        streams.append({"index": 2, "codec_type": "subtitle", "codec_name": "mov_text"})
    return {"streams": streams, "packets": [
        {"stream_index": 1, "pts": -1024, "dts": -1024, "duration": 1024, "data_hash": "SHA256:" + "a" * 64,
         "side_data_list": [{"side_data_type": "Skip Samples", "skip_samples": 1024,
                             "discard_padding": 0, "skip_reason": 0, "discard_reason": 0}]},
        {"stream_index": 1, "pts": 0, "dts": 0, "duration": 1024, "data_hash": "SHA256:" + "b" * 64},
    ]}


class PublishingPilotObservations(unittest.TestCase):
    def test_missing_capabilities_cannot_be_satisfied_by_headers_or_descriptions(self):
        filters = "Filters:\n T.. = Timeline support\n .. atrim A->A apad description\n .S astats A->A stats\n"
        self.assertEqual(pilot.feature_names("filters", filters), {"atrim", "astats"})
        self.assertNotIn("apad", pilot.feature_names("filters", filters))
        self.assertEqual(pilot.feature_names("muxers", " E null raw hash description\n"), {"null"})
        self.assertEqual(pilot.feature_names("demuxers", " D d lavfi Virtual device\n D mov,mp4 Movie\n"),
                         {"lavfi", "mov", "mp4"})
        protocols = pilot.feature_names("protocols", "Supported file protocols:\nInput:\n file\n pipe\nOutput:\n file\n")
        self.assertNotIn("output:pipe", protocols)

    def test_supplied_unicode_words_and_exact_intervals_survive_both_serializations(self):
        self.assertEqual(pilot.cues_from_text(pilot.MANUAL_SRT.decode()), pilot.EXPECTED_CUES)
        vtt = "WEBVTT\n\n" + pilot.MANUAL_SRT.decode().replace(",", ".")
        self.assertEqual(pilot.cues_from_text(vtt), pilot.EXPECTED_CUES)
        changed = vtt.replace("00:00:00.875", "00:00:00.876")
        self.assertNotEqual(pilot.cues_from_text(changed), pilot.EXPECTED_CUES)
        with self.assertRaises(ValueError):
            pilot.cues_from_text(vtt.replace("00:00:01.750", "invalid"))
        with tempfile.TemporaryDirectory() as directory:
            sidecar = Path(directory) / "lyrics.vtt"
            sidecar.write_bytes(vtt.encode())
            original = {sidecar: pilot.file_record(sidecar)}
            sidecar.write_bytes(vtt.replace("\n", "\r\n").encode())
            self.assertEqual(pilot.cues_from_text(sidecar.read_bytes().decode()), pilot.EXPECTED_CUES)
            with self.assertRaises(ValueError):
                pilot.require_preserved_files(original, "sidecar bytes changed")

    def test_video_clock_and_complete_stream_inventory_are_required(self):
        pilot.media_summary(probe(), False)
        for field, value in (("nb_read_frames", "63"), ("duration_ts", 32767), ("start_pts", 1)):
            bad = probe()
            bad["streams"][0][field] = value
            with self.subTest(field=field):
                with self.assertRaises(ValueError):
                    pilot.media_summary(bad, False)
        bad = probe()
        bad["streams"][1]["duration_ts"] -= 1
        with self.assertRaises(ValueError):
            pilot.media_summary(bad, False)
        bad = probe(True)
        bad["streams"].append(copy.deepcopy(bad["streams"][1]))
        with self.assertRaises(ValueError):
            pilot.media_summary(bad, True)

    def test_packet_hash_and_rational_clock_changes_cannot_match_preserved_audio(self):
        _, source = pilot.media_summary(probe(), False)
        final = probe(True)
        _, same = pilot.media_summary(final, True)
        self.assertEqual(source, same)
        for field, value in (("data_hash", "SHA256:" + "c" * 64), ("pts", 1), ("dts", 1)):
            changed = copy.deepcopy(final)
            changed["packets"][1][field] = value
            _, observed = pilot.media_summary(changed, True)
            self.assertNotEqual(source, observed)
        for change in ("configuration", "priming"):
            changed = copy.deepcopy(final)
            if change == "configuration":
                changed["streams"][1]["extradata_hash"] = "SHA256:" + "f" * 64
            else:
                changed["packets"][0]["side_data_list"][0]["skip_samples"] = 512
            _, observed = pilot.media_summary(changed, True)
            self.assertNotEqual(source, observed)
        final["streams"][1]["channels"] = 1
        with self.assertRaises(ValueError):
            pilot.media_summary(final, True)

    def test_process_success_cannot_replace_the_selected_cli_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stdout.json"
            path.write_text(json.dumps({"schema_version": 1, "command": "run", "status": "success", "result": {}}))
            self.assertEqual(pilot.machine_result(path, "run"), {})
            for command, status in (("resume", "success"), ("run", "error")):
                path.write_text(json.dumps({"schema_version": 1, "command": command, "status": status, "result": {}}))
                with self.assertRaises(ValueError):
                    pilot.machine_result(path, "run")
            path.write_text(json.dumps({"schema_version": 1, "command": "run", "status": "error",
                                        "error": {"category": "state", "message": "selected refusal"}}))
            self.assertEqual(pilot.machine_error(path, "run")["category"], "state")
            path.write_text("error: unknown argument")
            with self.assertRaises(ValueError):
                pilot.machine_error(path, "run")

    def test_existing_output_refuses_before_tool_execution_or_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            canary = root / "canary"
            canary.write_bytes(b"retain me")
            with self.assertRaises(ValueError):
                pilot.run(root, "/missing/aniflow", "/missing/ffmpeg", "/missing/ffprobe")
            self.assertEqual(canary.read_bytes(), b"retain me")
            videos = [root / "delivery.mp4", root / "master.mp4"]
            for path in videos:
                path.write_bytes(path.name.encode())
            originals = {path: pilot.file_record(path) for path in videos}
            pilot.require_preserved_files(originals, "accepted video changed")
            videos[1].write_bytes(b"changed master; delivery remains intact")
            with self.assertRaises(ValueError):
                pilot.require_preserved_files(originals, "accepted video changed")


if __name__ == "__main__":
    unittest.main()
