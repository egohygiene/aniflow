"""Synthetic unit fixtures for the narrow independent WAV acceptance boundary."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest
import wave


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("demucs_provider", ROOT / "providers/demucs/provider.py")
provider = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provider)


def write_wav(path: Path, *, rate: int = 44100, channels: int = 2, frames: int = 4410) -> None:
    with wave.open(str(path), "wb") as destination:
        destination.setnchannels(channels)
        destination.setsampwidth(2)
        destination.setframerate(rate)
        # Redistributable deterministic signed PCM samples, not real audio.
        destination.writeframes(struct.pack("<h", 1024) * channels * frames)


def riff(chunks: list[tuple[bytes, bytes]]) -> bytes:
    payload = b"WAVE" + b"".join(
        name + struct.pack("<I", len(data)) + data + (b"\x00" if len(data) % 2 else b"")
        for name, data in chunks
    )
    return b"RIFF" + struct.pack("<I", len(payload)) + payload


class PcmProfileTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.path = Path(self.temporary.name) / "synthetic source with spaces.wav"

    def reject_bytes(self, content: bytes, code: str) -> None:
        self.path.write_bytes(content)
        before = hashlib.sha256(content).hexdigest()
        with self.assertRaises(provider.ContractError) as caught:
            provider.inspect_pcm16_wav(self.path)
        self.assertEqual(caught.exception.code, code)
        self.assertEqual(provider.sha256_file(self.path), before)

    def test_valid_stereo_reports_exact_rational_duration_and_source_digest(self) -> None:
        write_wav(self.path)
        before = self.path.read_bytes()
        observation = provider.inspect_pcm16_wav(self.path, output=True)
        self.assertEqual(observation["duration"], {"numerator": 4410, "denominator": 44100})
        self.assertEqual(observation["sample_format"], "pcm_s16le")
        self.assertEqual(observation["sha256"], hashlib.sha256(before).hexdigest())
        self.assertEqual(self.path.read_bytes(), before)

    def test_mono_source_is_supported_but_not_a_demucs_stem(self) -> None:
        write_wav(self.path, rate=48000, channels=1, frames=4800)
        self.assertEqual(provider.inspect_pcm16_wav(self.path)["sample_rate_hz"], 48000)
        with self.assertRaises(provider.ContractError) as caught:
            provider.inspect_pcm16_wav(self.path, output=True)
        self.assertEqual(caught.exception.code, "invalid_output")

    def test_truncated_payload_and_trailing_bytes_are_refused(self) -> None:
        write_wav(self.path)
        content = self.path.read_bytes()
        for malformed in (content[:-1], content + b"unclaimed bytes"):
            with self.subTest(size=len(malformed)):
                self.reject_bytes(malformed, "invalid_audio")

    def test_pcm_float_extensible_and_invalid_layout_are_refused(self) -> None:
        for encoding, channels, bits in ((3, 2, 16), (65534, 2, 16), (1, 3, 16), (1, 2, 24)):
            with self.subTest(encoding=encoding, channels=channels, bits=bits):
                fmt = struct.pack("<HHIIHH", encoding, channels, 44100, 176400, 4, bits)
                self.reject_bytes(riff([(b"fmt ", fmt), (b"data", b"\x00" * 16)]), "unsupported_audio")

    def test_incomplete_sample_frame_and_wrong_byte_rate_are_refused(self) -> None:
        for byte_rate, payload in ((176401, b"\x00" * 16), (176400, b"\x00" * 15)):
            fmt = struct.pack("<HHIIHH", 1, 2, 44100, byte_rate, 4, 16)
            self.reject_bytes(riff([(b"fmt ", fmt), (b"data", payload)]), "invalid_audio")

    def test_duplicate_data_format_and_data_before_format_are_refused(self) -> None:
        fmt = struct.pack("<HHIIHH", 1, 2, 44100, 176400, 4, 16)
        for chunks in (
            [(b"fmt ", fmt), (b"data", b"\x00" * 16), (b"data", b"\x00" * 16)],
            [(b"fmt ", fmt), (b"fmt ", fmt), (b"data", b"\x00" * 16)],
            [(b"data", b"\x00" * 16), (b"fmt ", fmt)],
        ):
            self.reject_bytes(riff(chunks), "invalid_audio")

    def test_unknown_chunks_with_padding_and_pcm_extension_are_supported(self) -> None:
        fmt = struct.pack("<HHIIHHH", 1, 2, 44100, 176400, 4, 16, 0)
        self.path.write_bytes(riff([(b"JUNK", b"odd"), (b"fmt ", fmt), (b"data", b"\x00" * 16)]))
        self.assertEqual(provider.inspect_pcm16_wav(self.path)["sample_frames"], 4)

    def test_empty_audio_is_not_accepted(self) -> None:
        write_wav(self.path, frames=0)
        self.reject_bytes(self.path.read_bytes(), "invalid_audio")

    def test_audio_symlink_is_not_accepted(self) -> None:
        target = self.path.with_name("target.wav")
        write_wav(target)
        self.path.symlink_to(target)
        with self.assertRaises(provider.ContractError) as caught:
            provider.inspect_pcm16_wav(self.path)
        self.assertEqual(caught.exception.code, "invalid_file")

    def test_duration_tolerance_uses_exact_integer_cross_products(self) -> None:
        source = {"sample_frames": 48000, "sample_rate_hz": 48000}
        provider.require_duration_matches(source, {"sample_frames": 44982, "sample_rate_hz": 44100})
        with self.assertRaises(provider.ContractError) as caught:
            provider.require_duration_matches(source, {"sample_frames": 44983, "sample_rate_hz": 44100})
        self.assertEqual(caught.exception.code, "duration_mismatch")


class ContractTests(unittest.TestCase):
    def values(self) -> dict:
        return {
            "python_executable": "/configured venv/bin/python3",
            "python_sha256": "a" * 64,
            "ffmpeg_executable": "/configured tools/ffmpeg",
            "ffmpeg_sha256": "b" * 64,
            "ffprobe_executable": "/configured tools/ffprobe",
            "ffprobe_sha256": "e" * 64,
            "demucs_version": "4.0.1",
            "model_repository": "/external model cache",
            "model_bag_sha256": "c" * 64,
            "checkpoint_filename": "5c90dfd2-34c22ccb.th",
            "checkpoint_sha256": "d" * 64,
        }

    def test_schema_digest_is_the_actual_published_schema(self) -> None:
        schema = ROOT / "providers/demucs/configuration.schema.json"
        self.assertEqual(provider.CONFIGURATION_SCHEMA_SHA256, provider.sha256_file(schema))
        self.assertEqual(set(json.loads(schema.read_text())["required"]), provider.VALUE_KEYS)

    def test_unknown_configuration_and_unpinned_model_are_rejected(self) -> None:
        for values in ({**self.values(), "extra": True}, {**self.values(), "checkpoint_filename": "other.th"}):
            with self.assertRaises(provider.ContractError):
                provider.validate_values(values)

    def test_argument_array_keeps_paths_and_fixed_cpu_model_settings_exact(self) -> None:
        values = self.values()
        self.assertEqual(provider.separation_arguments(values, Path("/source with spaces.wav"), Path("/new outputs")), [
            "/configured venv/bin/python3", "-I", "-B", "-m", "demucs.separate",
            "--name", "htdemucs_6s", "--two-stems", "vocals", "--device", "cpu",
            "--shifts", "0", "--jobs", "0", "--repo", "/external model cache",
            "--out", "/new outputs", "--", "/source with spaces.wav",
        ])

    def test_offline_environment_has_explicit_flags_and_narrow_ffmpeg_path(self) -> None:
        environment = provider.offline_environment(self.values())
        self.assertEqual(environment["TORCH_HUB_OFFLINE"], "1")
        self.assertEqual(environment["HF_HUB_OFFLINE"], "1")
        self.assertEqual(environment["PATH"], "/configured tools")
        self.assertNotIn("PYTHONPATH", environment)

    def test_duplicate_invocation_fields_are_refused(self) -> None:
        with self.assertRaises(provider.ContractError):
            json.loads("{\"schema\":1,\"schema\":2}", object_pairs_hook=provider.reject_duplicate_fields)

    def test_ffprobe_must_share_the_explicit_ffmpeg_directory(self) -> None:
        with self.assertRaises(provider.ContractError) as caught:
            provider.validate_values({**self.values(), "ffprobe_executable": "/unrelated/ffprobe"})
        self.assertEqual(caught.exception.code, "invalid_path")

    def test_dependency_metadata_cannot_exceed_1024_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fake = Path(temporary) / "fake-python"
            fake.write_text("#!/usr/bin/python3\nprint(\"x\" * 2048)\n")
            fake.chmod(0o755)
            with self.assertRaises(provider.ContractError) as caught:
                provider.inspect_dependency_metadata({**self.values(), "python_executable": str(fake)})
            self.assertEqual(caught.exception.code, "dependency_unavailable")
            self.assertIn("1024", str(caught.exception))

    def test_existing_output_is_unchanged_and_refused_before_dependency_launch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.wav"
            write_wav(source)
            output = root / "outputs"
            output.mkdir()
            vocals = output / "vocals.wav"
            vocals.write_bytes(b"existing artifact must remain intact")
            values = self.values()  # Executables intentionally do not exist.
            configuration = {
                "schema": "aniflow.provider-configuration/v1",
                "provider": {"id": provider.PROVIDER_ID, "version": "1.0.0"},
                "capability": {"id": provider.CAPABILITY_ID, "version": "1.0.0"},
                "configuration_schema": {"id": provider.CONFIGURATION_SCHEMA_ID, "version": "1.0.0", "sha256": provider.CONFIGURATION_SCHEMA_SHA256},
                "values": values, "effective_configuration_sha256": provider.canonical_sha256(values),
            }
            def binding(port, path):
                return {"port": port, "artifact_id": port, "artifact_type": "audio/wav", "artifact_role": "temporal_component", "stream_role": "audio", "kind": "file", "path": str(path)}
            evidence = {"port": "evidence", "artifact_id": "evidence", "artifact_type": "application/vnd.aniflow.demucs-separation+json", "artifact_role": "validation_evidence", "kind": "file", "path": str(output / "evidence.json")}
            request = {
                "schema": "aniflow.provider-invocation/v1", "execution_semantics": "aniflow.provider-invocation/direct-argv/v1",
                "stage_id": "separate", "provider_lock_sha256": "a" * 64,
                "configuration": configuration, "inputs": [binding("audio", source)],
                "outputs": [binding("vocals", vocals), binding("accompaniment", output / "accompaniment.wav"), evidence],
            }
            with self.assertRaises(provider.ContractError) as caught:
                provider.execute(request)
            self.assertEqual(caught.exception.code, "output_exists")
            self.assertEqual(vocals.read_bytes(), b"existing artifact must remain intact")
            self.assertEqual(list(output.iterdir()), [vocals])


if __name__ == "__main__":
    unittest.main()
