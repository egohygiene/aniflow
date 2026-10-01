#!/usr/bin/env python3
"""Adversarial checks for the independent audio workflow evidence reader.

Only synthetic temporary files are created. These tests attack false support
claims, unresolved identities and clock/evidence drift rather than rerunning
the production pipeline or learned models.
"""
from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import wave

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("audio_workflow_checker", ROOT / "scripts/check-audio-workflow.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False) + "\n", encoding="utf-8")


def reference(path):
    return {"path": str(path), "sha256": CHECK.digest(path), "byte_size": path.stat().st_size}


class StrictJsonTests(unittest.TestCase):
    def test_duplicate_nested_field_is_rejected(self):
        with self.assertRaisesRegex(CHECK.ConformanceError, "Duplicate"):
            CHECK.loads('{"payload":{"sha256":"a","sha256":"b"}}')

    def test_method_cannot_borrow_another_type_implementation(self):
        sources = {"entry": "pub struct Present; pub struct Other; impl Other { pub fn validate() {} }"}
        self.assertFalse(CHECK.public_method(sources, "Present", "validate"))
        self.assertTrue(CHECK.public_method(sources, "Other", "validate"))

    def test_nonfinite_tokens_are_rejected(self):
        for token in ["NaN", "Infinity", "-Infinity", "1e999"]:
            with self.subTest(token=token), self.assertRaisesRegex(CHECK.ConformanceError, "Nonfinite"):
                CHECK.loads("{\"score\":" + token + "}")


class MatrixTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "matrix.json"
        self.matrix = CHECK.load(ROOT / CHECK.MATRIX)

    def check(self):
        write(self.path, self.matrix)
        return CHECK.check_matrix(ROOT, self.path)

    def test_repository_surface_resolves(self):
        result = self.check()
        self.assertEqual(result["acceptance_items"], 14)
        self.assertEqual(result["profiles"], 9)

    def test_missing_acceptance_item_is_rejected(self):
        self.matrix["acceptance_items"].pop(3)
        with self.assertRaisesRegex(CHECK.ConformanceError, "14 acceptance"):
            self.check()

    def test_unknown_checkpoint_is_rejected(self):
        self.matrix["acceptance_items"][0]["checkpoints"].append(999)
        with self.assertRaisesRegex(CHECK.ConformanceError, "Unknown acceptance checkpoint"):
            self.check()

    def test_nonexported_api_and_unknown_cli_task_schema_are_rejected(self):
        original = copy.deepcopy(self.matrix)
        for category, value, message in [
            ("types", "ImaginaryAudioDocument", "Missing public type"),
            ("cli", "audio plan --analysis imaginary", "Unknown analysis selector"),
            ("tasks", "audio:imaginary:ready", "Unknown Task"),
            ("schemas", "docs/contracts/imaginary.schema.json", "")]:
            with self.subTest(category=category):
                self.matrix = copy.deepcopy(original)
                profile = self.matrix["profiles"][0]
                if category == "types":
                    profile["public_api"][category].append(value)
                else:
                    profile[category].append(value)
                with self.assertRaises((CHECK.ConformanceError, FileNotFoundError)) as failure:
                    self.check()
                if message:
                    self.assertIn(message, str(failure.exception))

    def test_unsupported_scope_cannot_be_promoted(self):
        entry = next(value for value in self.matrix["explicit_dispositions"] if value["id"] == "wider_musical_families")
        entry["status"] = "implemented"
        with self.assertRaisesRegex(CHECK.ConformanceError, "scope was promoted"):
            self.check()

    def test_path_traversal_is_rejected(self):
        self.matrix["checkpoint_evidence"][0]["receipt"] = "../outside.json"
        with self.assertRaisesRegex(CHECK.ConformanceError, "Unsafe repository reference"):
            self.check()


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.source = self.root / "synthetic café.wav"
        with wave.open(str(self.source), "wb") as audio:
            audio.setparams((1, 2, 16000, 160, "NONE", "not compressed"))
            audio.writeframes(b"\0" * 320)
        self.binary = self.root / "binary-marker"
        self.binary.write_bytes(b"Synthetic identity marker; not executed\n")
        self.private = self.root / "private.json"
        write(self.private, {"private": "snapshot"})
        self.receipt_path = self.root / "receipt.json"
        self.receipt = {"schema": "aniflow.audio-workflow-smoke/v1", "synthetic_provider": True,
                        "real_model_inference": False, "models_downloaded": False, "sources_unchanged": True,
                        "source": reference(self.source), "aniflow": reference(self.binary),
                        "documents": [{"id": "private", **reference(self.private)}], "runs": [], "links": []}

    def check(self):
        write(self.receipt_path, self.receipt)
        return CHECK.check_receipt(ROOT, self.receipt_path)

    def public_document(self):
        path = self.root / "registry.json"
        schema = ROOT / "docs/contracts/timed-text-registry-v1.schema.json"
        example = CHECK.load(ROOT / "docs/contracts/examples/timed-text-registry-v1.example.json")
        write(path, example)
        item = {"id": "registry", **reference(path), "schema": example["schema"],
                "schema_path": str(schema), "schema_sha256": CHECK.digest(schema)}
        self.receipt["documents"].append(item)
        return path, item

    def test_private_snapshot_is_honestly_digest_only(self):
        result = self.check()
        self.assertEqual(result["digest_only_private"], 1)
        self.assertEqual(result["schema_validated"], 0)

    def test_document_bytes_cannot_be_changed(self):
        self.private.write_bytes(b"{}\n")
        with self.assertRaisesRegex(CHECK.ConformanceError, "Digest mismatch"):
            self.check()

    def test_document_size_is_checked_independently(self):
        self.receipt["documents"][0]["byte_size"] += 1
        with self.assertRaisesRegex(CHECK.ConformanceError, "Byte size mismatch"):
            self.check()

    def test_public_document_is_schema_validated(self):
        self.public_document()
        self.assertEqual(self.check()["schema_validated"], 1)

    def test_public_document_cannot_hide_as_digest_only(self):
        _, item = self.public_document()
        del item["schema_path"]
        del item["schema_sha256"]
        with self.assertRaisesRegex(CHECK.ConformanceError, "Missing/wrong public schema"):
            self.check()

    def test_schema_digest_drift_is_rejected(self):
        _, item = self.public_document()
        item["schema_sha256"] = "0" * 64
        with self.assertRaisesRegex(CHECK.ConformanceError, "Schema digest mismatch"):
            self.check()

    def test_unknown_tag_cannot_be_classified_private(self):
        write(self.private, {"schema": "aniflow.unqualified/v999"})
        self.receipt["documents"][0].update(reference(self.private), schema="aniflow.unqualified/v999")
        with self.assertRaisesRegex(CHECK.ConformanceError, "Unknown public document tag"):
            self.check()

    def test_rehashed_invalid_public_shape_is_rejected(self):
        path, item = self.public_document()
        value = CHECK.load(path)
        value["invented_field"] = True
        write(path, value)
        item.update(reference(path))
        with self.assertRaisesRegex(CHECK.ConformanceError, "Public schema validation failed"):
            self.check()

    def test_unknown_link_target_is_rejected(self):
        self.receipt["links"] = [{"id": "bad", "from_document": "private", "to_document": "absent",
                                   "relation": "digest_bound_upstream_evidence"}]
        with self.assertRaisesRegex(CHECK.ConformanceError, "unknown captured document"):
            self.check()

    def test_unbound_link_is_rejected(self):
        path, item = self.public_document()
        self.receipt["links"] = [{"id": "bad", "from_document": "private", "to_document": item["id"],
                                   "relation": "digest_bound_upstream_evidence"}]
        with self.assertRaisesRegex(CHECK.ConformanceError, "not bound by source report"):
            self.check()

    def test_false_model_qualification_is_rejected(self):
        self.receipt["real_model_inference"] = True
        with self.assertRaisesRegex(CHECK.ConformanceError, "Unsupported workflow qualification"):
            self.check()

    def test_machine_error_envelope_has_explicit_schema(self):
        schema = ROOT / "docs/contracts/machine-envelope-v1.schema.json"
        write(self.private, {"schema_version": 1, "command": "audio_plan", "status": "error",
                             "error": {"category": "dependency", "message": "Synthetic missing provider"}})
        self.receipt["documents"][0].update(reference(self.private), schema_path=str(schema), schema_sha256=CHECK.digest(schema))
        self.assertEqual(self.check()["schema_validated"], 1)

    def test_machine_envelope_cannot_hide_as_private(self):
        write(self.private, {"schema_version": 1, "command": "imaginary", "status": "error"})
        self.receipt["documents"][0].update(reference(self.private))
        with self.assertRaisesRegex(CHECK.ConformanceError, "Missing/wrong public schema"):
            self.check()

    def test_existing_internal_integrity_report_retains_actual_artifact(self):
        value = {"schema": "aniflow.artifact-integrity-validation/v1", "validation_id": "synthetic",
                 "contract": "aniflow.validation/artifact-integrity/v1", "artifact_id": "source_audio",
                 "artifact_sha256": CHECK.digest(self.source), "accepted": True}
        write(self.private, value)
        self.receipt["documents"][0].update(reference(self.private), schema=value["schema"])
        result = self.check()
        self.assertEqual(result["digest_and_semantics_internal_contract"], 1)
        self.assertEqual(result["schema_validated"], 0)
        value["artifact_sha256"] = "0" * 64
        write(self.private, value)
        self.receipt["documents"][0].update(reference(self.private))
        with self.assertRaisesRegex(CHECK.ConformanceError, "Internal integrity evidence differs"):
            self.check()

    def test_schema_registry_refuses_unresolved_network_resource(self):
        registry = CHECK.offline_registry(ROOT, {})
        with self.assertRaises(Exception) as error:
            registry.get_or_retrieve("https://invalid.example/forbidden.schema.json")
        self.assertIn("forbidden.schema.json", str(error.exception))

    def source_clock(self):
        return {"artifact": {"id": "source_audio", "sha256": CHECK.digest(self.source), "byte_size": self.source.stat().st_size},
                "stream_index": 0, "sample_rate_hz": 16000, "channels": 1, "frame_count": 160,
                "origin": {"numerator": 0, "denominator": 1}, "stem": None}

    def test_source_clock_is_checked_against_pcm_bytes(self):
        source = self.source_clock()
        files = {CHECK.digest(self.source): [self.source]}
        CHECK.verify_source(source, files)
        source["frame_count"] += 1
        with self.assertRaisesRegex(CHECK.ConformanceError, "Source PCM clock differs"):
            CHECK.verify_source(source, files)

    def test_unknown_normalized_artifact_reference_is_rejected(self):
        analysis = {"source": self.source_clock(), "artifacts": [], "providers": [],
                    "capabilities": [{"capability": {"id": "aniflow/audio-transcription"},
                                      "evidence_artifact_ids": ["imaginary"], "provider_evidence_ids": []}],
                    "observations": [], "timelines": [], "excerpts": [], "semantic_artifacts": []}
        with self.assertRaisesRegex(CHECK.ConformanceError, "Unknown capability artifact"):
            CHECK.verify_analysis(analysis, {CHECK.digest(self.source): [self.source]})

    def test_mido_readback_rejects_note_evidence_drift(self):
        import mido
        midi = mido.MidiFile(type=0, ticks_per_beat=960)
        track = mido.MidiTrack()
        midi.tracks.append(track)
        track.extend([mido.MetaMessage("set_tempo", tempo=500000),
                      mido.Message("program_change", program=0, channel=0),
                      mido.Message("note_on", note=60, velocity=80, channel=0),
                      mido.Message("note_off", note=60, velocity=0, channel=0, time=960)])
        path = self.root / "candidate.mid"
        midi.save(path)
        note = {"start_tick": 0, "end_tick": 960, "pitch": 60, "velocity": 80}
        report = {"read_back": {"notes": [note]}, "note_count": 1, "empty_candidate": False}
        notes = {"notes": [copy.deepcopy(note)]}
        CHECK.verify_midi(path, report, notes)
        notes["notes"][0]["pitch"] = 61
        with self.assertRaisesRegex(CHECK.ConformanceError, "Mido readback differs"):
            CHECK.verify_midi(path, report, notes)

    def test_package_candidate_cannot_use_another_retained_copy(self):
        package = self.root / "export"
        package.mkdir()
        candidate = self.root / "upstream-report.json"
        write(candidate, {"synthetic": "original"})
        write(package / "candidate-report.json", {"synthetic": "tampered"})
        notes, midi = package / "notes.json", package / "candidate.mid"
        write(notes, {"notes": []})
        midi.write_bytes(b"synthetic unparsed MIDI marker")
        artifact = lambda path: {"id": path.name, "sha256": CHECK.digest(path), "byte_size": path.stat().st_size}
        report = {"source_report": artifact(candidate), "source_audio": artifact(self.source),
                  "notes": artifact(notes), "midi": artifact(midi)}
        files = {CHECK.digest(path): [path] for path in [candidate, self.source, notes, midi]}
        # The correct original survives elsewhere; it cannot authorize a drifted
        # package-local candidate copy referenced by the completion marker.
        with self.assertRaisesRegex(CHECK.ConformanceError, "Digest mismatch: MIDI package:source_report"):
            CHECK.verify_midi_package(package, report, files)


class FixtureIndexTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "fixtures.json"
        self.index = CHECK.load(ROOT / "docs/validation/aniflow-51-fixture-index.json")

    def check(self):
        write(self.path, self.index)
        return CHECK.check_fixture_index(ROOT, self.path)

    def test_fixture_index_references_resolve(self):
        result = self.check()
        self.assertGreater(result["fixtures"], 0)

    def test_duplicate_global_identity_is_rejected(self):
        self.index["families"][1]["fixtures"][0]["id"] = self.index["families"][0]["fixtures"][0]["id"]
        with self.assertRaisesRegex(CHECK.ConformanceError, "Duplicate or invalid"):
            self.check()

    def test_missing_generator_is_rejected(self):
        self.index["families"][0]["generator"] = "scripts/imaginary-fixture.py"
        with self.assertRaises(FileNotFoundError):
            self.check()

    def test_declared_count_cannot_hide_missing_fixture(self):
        self.index["families"][0]["fixtures"].pop()
        with self.assertRaisesRegex(CHECK.ConformanceError, "family count differs"):
            self.check()


if __name__ == "__main__":
    unittest.main()
