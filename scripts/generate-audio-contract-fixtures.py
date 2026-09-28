#!/usr/bin/env python3
"""Generate deterministic, synthetic audio-analysis contract examples.

No media tools or models are executed. The source digest identifies PCM silence
created in memory. Other digests identify explicitly synthetic text payloads,
including stand-ins for provider implementations, model bytes and evidence. They
are contract examples, not successful measurements or release qualification.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import io
import json
import sys
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXAMPLES = ROOT / "docs" / "contracts" / "examples"
CAPABILITY_PREFIX = "aniflow/audio-"


def digest(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def synthetic_source() -> bytes:
    output = io.BytesIO()
    with wave.open(output, "wb") as audio:
        audio.setnchannels(2)
        audio.setsampwidth(2)
        audio.setframerate(48000)
        audio.writeframes(bytes(48000 * 2 * 2))
    return output.getvalue()


def synthetic_payload(identifier: str) -> bytes:
    return f"Synthetic aniflow contract fixture: {identifier}. No analyzer was run.\n".encode()


def artifact(identifier: str, content: bytes | None = None) -> dict:
    data = synthetic_payload(identifier) if content is None else content
    return {"id": identifier, "sha256": digest(data), "byte_size": len(data)}


def scope() -> dict:
    return {"channels": [0, 1], "stem_id": None}


def provenance(classification: str = "deterministic") -> dict:
    confidence = {"kind": "not_applicable"}
    if classification != "deterministic":
        confidence = {"kind": "uncalibrated", "score": 0.75}
    return {
        "class": classification,
        "confidence": confidence,
        "provider_evidence_id": "synthetic_provider",
        "evidence_artifact_ids": ["synthetic_evidence"],
    }


def capability(family: str, status: str = "complete") -> dict:
    return {
        "capability": {"id": CAPABILITY_PREFIX + family, "version": "1.0.0"},
        "status": status,
        "provider_evidence_ids": ["synthetic_provider"],
        "evidence_artifact_ids": ["synthetic_evidence"],
        "diagnostic_ids": [] if status == "complete" else ["provider_unavailable"],
    }


def base(family: str) -> dict:
    license_evidence = {"kind": "unavailable", "reason": "Synthetic identity only; no external provider or model was executed."}
    return {
        "schema": "aniflow.audio-analysis/v1",
        "status": "complete",
        "source": {
            "artifact": artifact("synthetic_source", synthetic_source()),
            "stream_index": 0,
            "sample_rate_hz": 48000,
            "channels": 2,
            "frame_count": 48000,
            "origin": {"numerator": 0, "denominator": 1},
            "stem": None,
        },
        "artifacts": [artifact("synthetic_evidence")],
        "providers": [{
            "id": "synthetic_provider",
            "provider": {"id": "org.egohygiene.aniflow.synthetic", "version": "1.0.0"},
            "implementation_sha256": digest(synthetic_payload("implementation")),
            "configuration_sha256": digest(b"{}"),
            "tools": [],
            "models": {"kind": "none_required"},
            "license": license_evidence,
        }],
        "capabilities": [capability(family)],
        "observations": [],
        "timelines": [],
        "semantic_artifacts": [],
        "excerpts": [],
        "diagnostics": [],
        "extensions": {
            "org.egohygiene.fixture": {
                "kind": "synthetic_contract_example",
                "source_recipe": "48000 sample frames of stereo PCM16 silence at 48000 Hz, RIFF WAV",
                "artifact_recipe": "Synthetic aniflow contract fixture: {id}. No analyzer was run.\\n (UTF-8; final newline)",
                "provider_executed": False,
                "model_executed": False,
                "claim": "All observations, identities and authority records are synthetic contract shapes, not observed analysis results.",
            }
        },
    }


def examples() -> dict[str, dict]:
    technical = base("technical-inspection")
    technical["observations"] = [
        {
            "id": "sample_rate",
            "capability_id": CAPABILITY_PREFIX + "technical-inspection",
            "kind": "sample_rate",
            "value": {"kind": "quantity", "value": 48000.0, "unit": "hertz"},
            "scope": scope(),
            "provenance": provenance(),
        },
        {
            "id": "channel_count",
            "capability_id": CAPABILITY_PREFIX + "technical-inspection",
            "kind": "channel_count",
            "value": {"kind": "count", "value": 2, "unit": "channels"},
            "scope": scope(),
            "provenance": provenance(),
        },
    ]
    estimated = base("musical-structure")
    estimated["observations"] = [
        {
            "id": "tempo_estimate",
            "capability_id": CAPABILITY_PREFIX + "musical-structure",
            "kind": "tempo",
            "value": {"kind": "quantity", "value": 120.0, "unit": "beats_per_minute"},
            "scope": scope(),
            "provenance": provenance("heuristic"),
        },
        {
            "id": "key_estimate",
            "capability_id": CAPABILITY_PREFIX + "musical-structure",
            "kind": "key",
            "value": {"kind": "label", "value": "C major (synthetic estimate)"},
            "scope": scope(),
            "provenance": {**provenance("probabilistic"), "confidence": {"kind": "unavailable", "reason": "The synthetic fixture has no calibrated confidence model."}},
        },
    ]
    unavailable = base("transcription")
    unavailable["status"] = "unavailable"
    unavailable["providers"][0]["models"] = {"kind": "unavailable", "reason": "No transcription model is installed for this synthetic fixture."}
    unavailable["capabilities"] = [capability("transcription", "unavailable")]
    unavailable["diagnostics"] = [{"id": "provider_unavailable", "code": "missing_model", "severity": "warning", "message": "Offline fixture: no model was downloaded or invoked."}]

    timeline = base("midi-extraction")
    timeline["capabilities"].extend([capability("timed-text"), capability("transcription")])
    timeline["artifacts"].extend(artifact(identifier) for identifier in ["midi_candidate", "reviewed_lyrics", "review_record", "observed_transcript", "excerpt_preview", "synthetic_license", "stem_relationship"])
    timeline["source"]["stem"] = {"id": "vocals", "original_mix": artifact("original_mix"), "relationship_evidence_id": "stem_relationship"}
    stem_scope = {"channels": [0, 1], "stem_id": "vocals"}
    timeline["timelines"] = [{
        "id": "polyphonic_candidates",
        "capability_id": CAPABILITY_PREFIX + "midi-extraction",
        "kind": "overlapping_regions",
        "scope": stem_scope,
        "events": [
            {"id": "note_c", "range": {"start": 0, "end": 24000}, "label": "candidate C4 (synthetic)", "provenance": provenance("probabilistic")},
            {"id": "note_e", "range": {"start": 12000, "end": 36000}, "label": "candidate E4 (synthetic)", "provenance": provenance("probabilistic")},
        ],
    }]
    timeline["semantic_artifacts"] = [
        {"artifact_id": "midi_candidate", "capability_id": CAPABILITY_PREFIX + "midi-extraction", "kind": "candidate_midi", "provenance": provenance("probabilistic"), "authority": None},
        {"artifact_id": "reviewed_lyrics", "capability_id": CAPABILITY_PREFIX + "timed-text", "kind": "reviewed_lyrics", "provenance": provenance(), "authority": {"supplied_by": "Synthetic reviewer fixture", "provenance_artifact_id": "review_record"}},
        {"artifact_id": "observed_transcript", "capability_id": CAPABILITY_PREFIX + "transcription", "kind": "observed_transcript", "provenance": provenance("probabilistic"), "authority": None},
    ]
    timeline["excerpts"] = [{"id": "source_excerpt", "artifact_id": "excerpt_preview", "range": {"start": 12000, "end": 24000}, "scope": stem_scope, "evidence_artifact_ids": ["synthetic_evidence"]}]
    component = {"id": "synthetic_component", "version": "fixture-1", "revision": "synthetic-revision-1", "sha256": digest(synthetic_payload("component")), "license": {"kind": "recorded", "statement": "Synthetic license record; no legal determination.", "evidence_artifact_id": "synthetic_license"}}
    timeline["providers"][0]["tools"] = [copy.deepcopy(component)]
    timeline["providers"][0]["models"] = {"kind": "available", "components": [component]}
    return {
        f"audio-analysis-{name}-v1.example.json": document
        for name, document in [("technical", technical), ("estimated", estimated), ("unavailable", unavailable), ("timeline", timeline)]
    }


def encoded(document: dict) -> str:
    return json.dumps(document, ensure_ascii=False, indent=2, allow_nan=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Refuse stale checked-in fixtures without writing files.")
    args = parser.parse_args()
    stale = []
    for filename, document in examples().items():
        path = EXAMPLES / filename
        content = encoded(document)
        if args.check:
            if not path.is_file() or path.read_text(encoding="utf-8") != content:
                stale.append(filename)
        else:
            path.write_text(content, encoding="utf-8")
    if stale:
        print("Audio contract fixtures are stale: " + ", ".join(stale), file=sys.stderr)
        return 1
    print(f"{'Checked' if args.check else 'Generated'} {len(examples())} synthetic audio contract examples.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
