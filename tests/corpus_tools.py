#!/usr/bin/env python3
"""Authored generator, drift and ownership regressions; synthetic data only."""

import binascii
import copy
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import time
import unittest
from unittest import mock
import wave
import zlib

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from corpus import catalog
from corpus.recipes import json_bytes, recipes
from corpus.storage import digest, inventory, relative_path, safe_new_directory
from corpus.process import execute


class CorpusTools(unittest.TestCase):
    def test_catalog_is_deterministic_and_source_bound(self):
        first = catalog.catalog_bytes()
        self.assertEqual(first, catalog.catalog_bytes())
        self.assertEqual(first, catalog.CATALOG.read_bytes())
        value = json.loads(first)
        self.assertEqual(len({item["id"] for item in value["cases"]}), len(value["cases"]))
        self.assertEqual({item["validity"] for item in value["cases"]},
                         {"valid", "boundary", "malformed", "unsupported", "resource_stress"})
        for source in value["sources"]:
            self.assertEqual(digest((ROOT / source["path"]).read_bytes()), source["sha256"])

    def test_generated_bundle_matches_exact_inventory_and_repeat_generation(self):
        with tempfile.TemporaryDirectory() as directory:
            first, second = Path(directory) / "first", Path(directory) / "second"
            catalog.generate(first)
            catalog.generate(second)
            catalog.verify_bundle(first)
            self.assertEqual(inventory(first), inventory(second))
            self.assertEqual(catalog.CATALOG.read_bytes(), catalog.catalog_bytes())

    def test_drift_check_refuses_without_rewriting(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "catalog.json"
            data = copy.deepcopy(catalog.compile_catalog())
            data["cases"][0]["inventory"]["files"][0]["sha256"] = "0" * 64
            path.write_bytes(json_bytes(data))
            before = path.read_bytes()
            with mock.patch.object(catalog, "CATALOG", path), self.assertRaises(ValueError):
                catalog.check_catalog()
            self.assertEqual(path.read_bytes(), before)

    def test_bundle_rejects_missing_extra_and_changed_payloads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "bundle"
            data = catalog.generate(root)
            case = next(item for item in data["cases"] if item["inventory"]["files"])
            path = root / case["id"] / case["inventory"]["files"][0]["path"]
            original = path.read_bytes()
            path.write_bytes(b"tamper")
            with self.assertRaises(ValueError):
                catalog.verify_bundle(root)
            path.write_bytes(original)
            path.unlink()
            with self.assertRaises(ValueError):
                catalog.verify_bundle(root)
            path.write_bytes(original)
            (root / "unknown").write_bytes(b"extra")
            with self.assertRaises(ValueError):
                catalog.verify_bundle(root)

    def test_output_ownership_and_paths_fail_closed(self):
        for value in ["../escape", "/absolute", "a/../b", "a//b", "a\\b", "C:/file", "a/./b"]:
            with self.assertRaises(ValueError, msg=value):
                relative_path(value)
        with tempfile.TemporaryDirectory() as directory:
            existing = Path(directory) / "existing"
            existing.mkdir()
            (existing / "sentinel").write_bytes(b"unchanged")
            with self.assertRaises(ValueError):
                catalog.generate(existing)
            self.assertEqual((existing / "sentinel").read_bytes(), b"unchanged")
            with self.assertRaises(ValueError):
                safe_new_directory(ROOT / "generated-media", ROOT)

    @unittest.skipUnless(sys.platform != "win32", "Unix symlink fixture")
    def test_links_are_never_followed_or_adopted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            (root / "link").symlink_to(target, target_is_directory=True)
            with self.assertRaises(ValueError):
                catalog.generate(root / "link" / "output")
            (target / "linked-file").symlink_to(ROOT / "LICENSE")
            with self.assertRaises(ValueError):
                inventory(target)

    def test_pcm_recipes_have_exact_sample_count_rate_width_and_payload(self):
        for recipe in recipes():
            if recipe.family != "audio" or recipe.oracle["outcome"] != "decode":
                continue
            data = next(iter(recipe.files.values()))
            with wave.open(io.BytesIO(data), "rb") as source:
                self.assertEqual(source.getnchannels(), recipe.parameters["channels"])
                self.assertEqual(source.getframerate(), recipe.parameters["sample_rate"])
                self.assertEqual(source.getsampwidth() * 8, recipe.parameters["bits"])
                self.assertEqual(source.getnframes(), recipe.parameters["frames"])
                self.assertEqual(len(source.readframes(source.getnframes())),
                                 source.getnframes() * source.getnchannels() * source.getsampwidth())

    def test_png_recipes_have_valid_crc_and_exact_bounded_scanlines(self):
        for recipe in recipes():
            if recipe.family != "image" or recipe.oracle["outcome"] != "decode":
                continue
            data = next(iter(recipe.files.values()))
            self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
            offset, pixels, dimensions = 8, b"", None
            while offset < len(data):
                length = struct.unpack(">I", data[offset:offset + 4])[0]
                kind = data[offset + 4:offset + 8]
                payload = data[offset + 8:offset + 8 + length]
                crc = struct.unpack(">I", data[offset + 8 + length:offset + 12 + length])[0]
                self.assertEqual(crc, binascii.crc32(kind + payload) & 0xffffffff)
                if kind == b"IHDR":
                    dimensions = struct.unpack(">IIBBBBB", payload)
                if kind == b"IDAT":
                    pixels += payload
                offset += length + 12
            self.assertEqual(offset, len(data))
            width, height, depth, color, _, _, _ = dimensions
            self.assertEqual(depth, 8)
            decoded = zlib.decompress(pixels)
            stride = width * (4 if color == 6 else 3) + 1
            self.assertEqual(len(decoded), stride * height)
            self.assertTrue(all(decoded[row * stride] == 0 for row in range(height)))

    def test_inventory_order_is_independent_of_recipe_insertion_order(self):
        recipe = next(item for item in recipes() if item.name == "frames.lexical-order")
        first = catalog.describe(recipe)
        recipe.files = dict(reversed(list(recipe.files.items())))
        self.assertEqual(first, catalog.describe(recipe))
        self.assertEqual(first["oracle"]["properties"]["lexical_order"],
                         [item["path"] for item in first["inventory"]["files"]])

    def test_process_failure_and_missing_tool_remain_nonpassing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, command, state in [
                ("failure", [sys.executable, "-c", "raise SystemExit(23)"], "failed"),
                ("missing", [str(root / "missing-program")], "unavailable"),
            ]:
                result = execute(command, cwd=root, logs=root / "logs", name=name,
                                 deadline=time.monotonic() + 5)
                self.assertEqual(result["state"], state)
                self.assertTrue((root / "logs" / result["stderr"]["path"]).is_file())

    def test_process_timeout_stops_bounded_work(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = execute([sys.executable, "-c", "import time; time.sleep(10)"],
                             cwd=root, logs=root / "logs", name="timeout", deadline=time.monotonic() + 0.1)
            self.assertEqual(result["state"], "timed_out")
            self.assertLess(result["elapsed_seconds"], 5)

    def test_capture_limit_cannot_become_success_after_fast_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = execute([sys.executable, "-c", "import sys; sys.stdout.write(\"x\" * (5 * 1024 * 1024))"],
                             cwd=root, logs=root / "logs", name="capture", deadline=time.monotonic() + 5)
            self.assertEqual(result["state"], "failed")
            self.assertIn("capture limit", result["message"])


if __name__ == "__main__":
    unittest.main()
