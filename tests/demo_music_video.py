#!/usr/bin/env python3
"""Synthetic demo regressions; no encoder, provider, model or network is used.

These cases are authored for later execution under #64. Importing the demo
module only exposes its pure recipes and admission helpers.
"""

import copy
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("aniflow_demo_music_video", ROOT / "scripts/demo-music-video.py")
demo = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = demo
SPEC.loader.exec_module(demo)


def observation(width=320, height=180):
    video = {
        "index": 0, "codec_type": "video", "codec_name": "h264",
        "width": width, "height": height, "pix_fmt": "yuv420p",
        "field_order": "progressive", "sample_aspect_ratio": "1:1",
        "color_range": "tv", "color_space": "bt709",
        "color_transfer": "bt709", "color_primaries": "bt709",
        "r_frame_rate": "24/1", "avg_frame_rate": "24/1",
        "time_base": "1/24000", "start_pts": 0,
        "duration_ts": 48000, "nb_read_frames": "48",
    }
    frames = [{
        "media_type": "video", "stream_index": 0,
        "pts": ordinal * 1000, "duration": 1000,
        "width": width, "height": height, "pix_fmt": "yuv420p",
        "sample_aspect_ratio": "1:1", "interlaced_frame": 0,
        "crop_top": 0, "crop_bottom": 0, "crop_left": 0, "crop_right": 0,
        "color_range": "tv", "color_space": "bt709",
        "color_transfer": "bt709", "color_primaries": "bt709",
    } for ordinal in range(48)]
    return {"streams": [video], "frames": frames, "chapters": [],
            "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2",
                       "start_time": "0.000000", "duration": "2.000000"}}


class MusicVideoDemo(unittest.TestCase):
    def test_raw_sources_have_exact_repeatable_clock_planes_and_visible_motion(self):
        for width, height in [(320, 180), (180, 320)]:
            with self.subTest(width=width, height=height):
                data = demo.y4m_bytes(width, height)
                self.assertEqual(data, demo.y4m_bytes(width, height))
                header, payload = data.split(b"\n", 1)
                fields = header.split()
                self.assertEqual(fields[0], b"YUV4MPEG2")
                for token in [f"W{width}", f"H{height}", "F24:1", "Ip", "A1:1"]:
                    self.assertIn(token.encode(), fields)
                self.assertTrue(any(token.startswith(b"C420") for token in fields))
                luma_size = width * height
                frame_size = luma_size * 3 // 2
                stride = len(b"FRAME\n") + frame_size
                self.assertEqual(len(payload), 48 * stride)
                frames = []
                for ordinal in range(48):
                    block = payload[ordinal * stride:(ordinal + 1) * stride]
                    self.assertEqual(block[:6], b"FRAME\n")
                    frame = block[6:]
                    self.assertEqual(len(frame), frame_size)
                    self.assertTrue(all(16 <= value <= 235 for value in frame[:luma_size]))
                    self.assertTrue(all(16 <= value <= 240 for value in frame[luma_size:]))
                    frames.append(frame)
                self.assertGreater(len(set(frames)), 1, "the viewing fixture must show motion")

    def test_probe_admission_refuses_changed_clock_geometry_count_audio_and_color(self):
        valid = observation()
        demo.validate_observation(valid, 320, 180)
        mutations = [
            ("stream shape", lambda value: value["streams"].__setitem__(0, None)),
            ("frame shape", lambda value: value["frames"].__setitem__(0, None)),
            ("container shape", lambda value: value.update(format=None)),
            ("frame count", lambda value: value["frames"].pop()),
            ("declared count", lambda value: value["streams"][0].update(nb_read_frames="47")),
            ("geometry", lambda value: value["streams"][0].update(width=318)),
            ("first frame", lambda value: value["frames"][0].update(pts=1)),
            ("duplicate time", lambda value: value["frames"][1].update(pts=0)),
            ("frame duration", lambda value: value["frames"][1].update(duration=999)),
            ("stream duration", lambda value: value["streams"][0].update(duration_ts=47000)),
            ("frame rate", lambda value: value["streams"][0].update(avg_frame_rate="30/1")),
            ("pixel format", lambda value: value["streams"][0].update(pix_fmt="yuv420p10le")),
            ("range", lambda value: value["streams"][0].update(color_range="pc")),
            ("primaries", lambda value: value["streams"][0].update(color_primaries="bt2020")),
            ("aspect", lambda value: value["streams"][0].update(sample_aspect_ratio="2:1")),
            ("interlacing", lambda value: value["frames"][0].update(interlaced_frame=1)),
            ("cropping", lambda value: value["frames"][0].update(crop_top=2)),
            ("audio", lambda value: value["streams"].append({"index": 1, "codec_type": "audio"})),
        ]
        for name, mutate in mutations:
            with self.subTest(name=name):
                changed = copy.deepcopy(valid)
                mutate(changed)
                with self.assertRaises(ValueError):
                    demo.validate_observation(changed, 320, 180)
        with self.assertRaises(ValueError):
            demo.validate_observation(None, 320, 180)

    def test_source_configuration_binds_selected_local_tools_and_needs_no_neural_assets(self):
        tools = {name: {"requested_path": f"/synthetic/requested/{name}",
                        "resolved_path": f"/synthetic/local tools/{name}",
                        "sha256": digest * 64, "version": f"{name} version 8.0.1 synthetic"}
                 for name, digest in [("ffmpeg", "a"), ("ffprobe", "b")]}
        before = copy.deepcopy(tools)
        configuration = demo.source_configuration(tools)
        self.assertEqual(tools, before)
        self.assertEqual(configuration["schema"], "aniflow.source-music-video-recipe/v1")
        self.assertEqual(configuration["extraction"]["source_selection"],
                         {"video_stream": 0, "audio_stream": None, "no_audio": True, "discard_streams": []})
        recipe = configuration["recipe"]
        self.assertEqual(recipe["schema"], "aniflow.music-video-recipe/v1")
        for mode in ["repair", "upscale"]:
            self.assertEqual(recipe[mode]["mode"], "disabled")
            self.assertTrue(recipe[mode]["reason"].strip())
        for owner in [configuration["extraction"], recipe["assembly"]]:
            for name in ["ffmpeg", "ffprobe"]:
                self.assertEqual(owner[name], {"executable": tools[name]["resolved_path"],
                                              "version": "8.0.1", "sha256": tools[name]["sha256"]})
        self.assertEqual(recipe["repair"]["import"]["ffmpeg"], configuration["extraction"]["ffmpeg"])

    def test_encoding_preserves_literal_paths_and_refuses_overwrite(self):
        tool = Path("/synthetic/tool dir/ffmpeg")
        source = Path("/synthetic/source $(do-not-execute); file.y4m")
        destination = Path("/synthetic/output `literal`; file.mp4")
        arguments = demo.encode_arguments(tool, source, destination)
        self.assertIsInstance(arguments, list)
        arguments = list(map(str, arguments))
        self.assertEqual(arguments[0], str(tool))
        self.assertEqual(arguments[arguments.index("-i") + 1], str(source))
        self.assertEqual(arguments[-1], str(destination))
        self.assertIn("-n", arguments)
        self.assertNotIn("-y", arguments)
        self.assertIn("-an", arguments)

    def test_invalid_output_locations_refuse_before_tool_execution_and_preserve_existing_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            existing = root / "existing"
            existing.mkdir()
            sentinel = existing / "sentinel"
            sentinel.write_bytes(b"preserve this existing directory")
            refused = [existing, demo.ROOT / "__synthetic_demo_output__"]
            if sys.platform != "win32":
                (root / "linked-parent").symlink_to(existing, target_is_directory=True)
                refused.append(root / "linked-parent" / "output")
            with mock.patch.object(demo, "tool_identity") as launch:
                for path in refused:
                    with self.subTest(path=path), self.assertRaises(ValueError):
                        demo.generate(path, Path("/synthetic/ffmpeg"), Path("/synthetic/ffprobe"))
                launch.assert_not_called()
            self.assertEqual(sentinel.read_bytes(), b"preserve this existing directory")
            self.assertEqual(sorted(path.name for path in existing.iterdir()), ["sentinel"])


if __name__ == "__main__":
    unittest.main()
