#!/usr/bin/env python3
"""Synthetic adapter tests; no Basic Pitch packages, weights or inference."""

import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch
import wave


ROOT = Path(__file__).resolve().parents[1]
ADAPTER_PATH = ROOT / "scripts" / "audio-midi-adapter.py"
SPEC = importlib.util.spec_from_file_location("audio_midi_adapter", ADAPTER_PATH)
ADAPTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADAPTER)


def note(start=0.1, end=0.2, pitch=60, amplitude=0.5, bends=None):
    return start, end, pitch, amplitude, bends


def synthetic_wave(path, *, channels=1, rate=22050, silence=False):
    with wave.open(str(path), "wb") as output:
        output.setnchannels(channels)
        output.setsampwidth(2)
        output.setframerate(rate)
        output.writeframes(struct.pack("<h", 0 if silence else 1024) * rate * channels)


class FakeArray:
    def __init__(self, rows, columns, value=0.5):
        self.shape = rows, columns
        self.flat = iter([value] * rows * columns)


class AudioMidiAdapterTests(unittest.TestCase):
    def refusal(self, function, *arguments, code=None):
        with self.assertRaises(ADAPTER.Refusal) as caught:
            function(*arguments)
        if code is not None:
            self.assertEqual(caught.exception.code, code)

    def test_polyphonic_notes_sort_and_native_velocity_ties(self):
        values = [note(0.2, 0.3, 64, 1), note(0.1, 0.25, 67, 0.5), note(0.1, 0.2, 60, 0.5)]
        actual = ADAPTER.normalize_events(values, 22050)
        self.assertEqual([(n["start_microseconds"], n["pitch"], n["velocity"]) for n in actual],
                         [(100000, 60, 64), (100000, 67, 64), (200000, 64, 127)])
        self.assertTrue(all(n["activation"] in (0.5, 1) for n in actual))

    def test_half_up_microseconds_and_touching_same_pitch(self):
        actual = ADAPTER.normalize_events([note(0.1000005, 0.2), note(0.2, 0.3)], 22050)
        self.assertEqual(actual[0]["start_microseconds"], 100001)
        self.assertEqual(actual[0]["end_microseconds"], actual[1]["start_microseconds"])
        self.assertEqual(ADAPTER.microseconds(0.0078125), 7813)  # Exact binary half tie.
        for value in (0.0, 0.1, 0.1000005, 0.0078125, 119.99999949999999):
            numerator, denominator = value.as_integer_ratio()
            error = abs(ADAPTER.microseconds(value) * denominator - numerator * 1000000)
            self.assertLessEqual(2 * error, denominator)

    def test_timing_refusals_precede_rounding(self):
        for event in (note(-0.0000001), note(0.2, 0.2), note(0.1, 1.0000001),
                      note(0.1000001, 0.1000002), note(float("nan")), note(0.1, float("inf"))):
            with self.subTest(event=event):
                self.refusal(ADAPTER.normalize_events, [event], 22050)

    def test_pitch_activation_velocity_and_bend_refusals(self):
        for event in (note(pitch=20), note(pitch=109), note(pitch=60.5), note(pitch=True),
                      note(amplitude=-0.1), note(amplitude=1.1), note(amplitude=float("nan")),
                      note(amplitude=0), note(bends=[]), note(bends=[0])):
            with self.subTest(event=event):
                self.refusal(ADAPTER.normalize_events, [event], 22050)

    def test_same_pitch_overlap_duplicate_and_count_refusals(self):
        for events in ([note(0.1, 0.4), note(0.3, 0.5)], [note(), note()], [note()] * 8193):
            self.refusal(ADAPTER.normalize_events, events, 22050)
        self.assertEqual(ADAPTER.normalize_events([], 22050), [])

    def test_output_shape_and_nonfinite_activations(self):
        good = lambda: {name: FakeArray(86, columns) for name, columns in
                        (("note", 88), ("onset", 88), ("contour", 264))}
        ADAPTER.validate_outputs(good(), 22050)
        for replacement in (FakeArray(85, 88), FakeArray(86, 88, float("nan")), FakeArray(86, 88, 1.1)):
            value = good()
            value["note"] = replacement
            self.refusal(ADAPTER.validate_outputs, value, 22050)
        value = good()
        value["extra"] = []
        self.refusal(ADAPTER.validate_outputs, value, 22050)

    def test_synthetic_wave_admission_preserves_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.wav"
            synthetic_wave(path)
            before = path.read_bytes()
            self.assertEqual(ADAPTER.inspect_wave(path), 22050)
            self.assertEqual(path.read_bytes(), before)
            for settings in ({"channels": 2}, {"rate": 16000}, {"silence": True}):
                synthetic_wave(path, **settings)
                self.refusal(ADAPTER.inspect_wave, path)

    def test_truncation_noncanonical_and_model_identity_refusal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "source.wav"
            synthetic_wave(path)
            path.write_bytes(path.read_bytes()[:-1])
            self.refusal(ADAPTER.inspect_wave, path)
            link = root / "link.wav"
            link.symlink_to(path)
            self.refusal(ADAPTER.inspect_wave, link)
            model = root / "model.onnx"
            model.write_bytes(b"synthetic marker")
            self.refusal(ADAPTER.verify_model, model, code="unsupported_model")
            model.write_bytes(b"\0" * ADAPTER.MODEL_BYTES)
            self.refusal(ADAPTER.verify_model, model, code="unsupported_model")

    def test_runtime_inventory_detects_unrecorded_file_and_refuses_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            packages = root / "lib" / "site-packages"
            packages.mkdir(parents=True)
            path = packages / "module.py"
            path.write_text("# synthetic runtime\n")
            fake_distribution = types.SimpleNamespace(files=[Path("module.py")], locate_file=lambda entry: packages / entry)
            with patch.object(sys, "prefix", str(root)), \
                    patch.object(ADAPTER.importlib.metadata, "distributions", return_value=[fake_distribution]), \
                    patch.object(ADAPTER.sysconfig, "get_path", return_value=str(packages)):
                first = ADAPTER.runtime_manifest()
                self.assertEqual(len(first), 1)
                added = packages / "unrecorded.py"
                added.write_text("# newly added synthetic file\n")
                second = ADAPTER.runtime_manifest()
                self.assertEqual(len(second), 2)
                self.assertNotEqual(first, second)
                (packages / "link.py").symlink_to(path)
                self.refusal(ADAPTER.runtime_manifest, code="runtime_path_escape")

    def test_unsupported_runtime_and_python_refuse_before_imports(self):
        distribution = lambda name: types.SimpleNamespace(version=ADAPTER.VERSIONS[name])
        with patch.object(sys, "version_info", (3, 11)), \
                patch.object(ADAPTER.importlib.metadata, "distribution", side_effect=distribution), \
                patch.object(ADAPTER.importlib.util, "find_spec", return_value=object()):
            self.refusal(ADAPTER.installed_distributions, code="unsupported_runtime")
        with patch.object(sys, "version_info", (3, 12)):
            self.refusal(ADAPTER.installed_distributions, code="unsupported_python")

    def test_exact_api_settings_with_stub_modules(self):
        calls = {}

        class Model:
            MODEL_TYPES = types.SimpleNamespace(ONNX="onnx")

            def __init__(self, path):
                calls["model"] = path
                self.model_type = "onnx"
                self.model = types.SimpleNamespace(get_providers=lambda: ["CPUExecutionProvider"])

        def infer(path, model):
            calls["input"] = path
            return {name: FakeArray(86, columns) for name, columns in
                    (("note", 88), ("onset", 88), ("contour", 264))}

        def decode(outputs, **settings):
            calls["settings"] = settings
            return object(), [note(0.2, 0.3, 64), note(0.1, 0.25, 60)]

        modules = {
            "onnxruntime": types.SimpleNamespace(disable_telemetry_events=lambda: None),
            "basic_pitch": types.ModuleType("basic_pitch"),
            "basic_pitch.inference": types.SimpleNamespace(Model=Model, run_inference=infer),
            "basic_pitch.note_creation": types.SimpleNamespace(model_output_to_notes=decode),
        }
        with tempfile.TemporaryDirectory() as directory:
            path, model = Path(directory) / "source.wav", Path(directory) / "model.onnx"
            synthetic_wave(path)
            before = hashlib.sha256(path.read_bytes()).hexdigest()
            # Stub-only invocation: neither installed packages nor actual model
            # admission/inference are exercised by this contract test.
            with patch.dict(sys.modules, modules), patch.object(ADAPTER, "verify_model"), \
                    patch.object(ADAPTER, "installed_distributions", return_value={}):
                result = ADAPTER.analyze(path, model)
            self.assertEqual(calls["input"], path)
            self.assertEqual(calls["model"], model)
            self.assertEqual(calls["settings"], {
                "onset_thresh": 0.5, "frame_thresh": 0.3, "infer_onsets": True,
                "min_note_len": 11, "min_freq": None, "max_freq": None,
                "include_pitch_bends": False, "multiple_pitch_bends": False,
                "melodia_trick": True, "midi_tempo": 120,
            })
            self.assertEqual(result["duration_microseconds"], 1000000)
            self.assertEqual([n["pitch"] for n in result["notes"]], [60, 64])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), before)
            # A successful constructor is insufficient if it selects a GPU.
            original_init = Model.__init__

            def wrong_backend(self, model_path):
                original_init(self, model_path)
                self.model = types.SimpleNamespace(get_providers=lambda: ["CUDAExecutionProvider"])

            with patch.dict(sys.modules, modules), patch.object(ADAPTER, "verify_model"), \
                    patch.object(ADAPTER, "installed_distributions", return_value={}), \
                    patch.object(Model, "__init__", wrong_backend):
                self.refusal(ADAPTER.analyze, path, model, code="unsupported_runtime")

    def test_cli_rejects_nonisolated_invocation_and_conflicting_flags(self):
        result = subprocess.run([sys.executable, str(ADAPTER_PATH), "--probe"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 65)
        self.assertEqual(json.loads(result.stderr)["code"], "isolation_required")
        for arguments in (["--probe", "--model", "/tmp/model.onnx"], ["--input", "/tmp/source.wav"]):
            result = subprocess.run([sys.executable, "-I", "-B", str(ADAPTER_PATH), *arguments], capture_output=True)
            self.assertEqual(result.returncode, 2)


if __name__ == "__main__":
    unittest.main()
