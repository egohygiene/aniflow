#!/usr/bin/env python3
"""Exercise reviewed-lyrics alignment with generated PCM and a synthetic native ABI.

Real ffmpeg/ffprobe inspect generated tones. Marker acoustic resources and a local
fixture executable test contracts only; no model is downloaded or executed, and
no review authority or alignment accuracy is independently established.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import struct
import subprocess
import sys
import tempfile
import wave


INSPECTION = runpy.run_path(str(Path(__file__).with_name("smoke-audio-inspection.py")))
sha256 = INSPECTION["sha256"]
local_tool_pin = INSPECTION["local_tool_pin"]
MODEL_FILES = ["feat.params", "mdef", "means", "noisedict", "sendump", "transition_matrices", "variances"]
# The admitted feature/noise declarations are fixtures, not trained resources.
FEATURE_PARAMETERS = """-lowerf 130
-upperf 6800
-nfilt 25
-transform dct
-lifter 22
-feat 1s_c_d_dd
-svspec 0-12/13-25/26-38
-agc none
-cmn batch
-varnorm no
-model ptm
-remove_noise yes
"""
NOISE_DICTIONARY = """<s> SIL
</s> SIL
<sil> SIL
[NOISE] +NSN+
[SPEECH] +SPN+
"""
POCKETSPHINX = r'''
import json, pathlib, sys
root = pathlib.Path(__FIXTURE_ROOT__)
args = sys.argv[1:]
stage = pathlib.Path(sys.argv[0]).resolve().parent
phrase = (root / "phrase").read_text()
assert args == [
    "-hmm", str(stage / "model"), "-dict", str(stage / "dictionary.dict"),
    "-lm", str(stage / "disabled.lm"),
    "-samprate", "16000", "-frate", "100", "-phone_align", "no",
    "-state_align", "no", "-fsgusealtpron", "no", "-loglevel", "ERROR",
    "align", str(stage / "source.wav"), phrase,
]
assert pathlib.Path.cwd() == stage
assert not (stage / "disabled.lm").exists()
assert sorted(path.name for path in (stage / "model").iterdir()) == [
    "feat.params", "mdef", "means", "noisedict", "sendump", "transition_matrices", "variances",
]
with (root / "launches").open("a") as log:
    log.write(json.dumps(args) + "\n")
words = [
    {"b": 0.0, "d": 0.4, "p": 1.0, "t": "hello"},
    {"b": 0.5, "d": 0.4, "p": 1.0, "t": "synthetic"},
    {"b": 1.0, "d": 0.4, "p": 1.0, "t": "world"},
]
mode = (root / "mode").read_text()
if mode == "partial": words = [words[0], words[2]]
if mode == "ambiguous": words = [words[0]]
print(json.dumps({"b": 0.0, "d": 2.0, "p": 1.0,
                  "t": " ".join(word["t"] for word in words), "w": words}))
'''


def invoke(binary, command, *arguments, success=True):
    result = subprocess.run(
        [str(binary), "--output", "json", *map(str, arguments)],
        capture_output=True, text=True, timeout=60, check=False,
    )
    if (result.returncode == 0) != success:
        raise AssertionError(f"{command}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
    envelope = json.loads(result.stdout if success else result.stderr)
    assert envelope["schema_version"] == 1 and envelope["command"] == command
    assert envelope["status"] == ("success" if success else "error")
    return envelope.get("result")


def reference(identifier, content):
    return {"id": identifier, "sha256": hashlib.sha256(content).hexdigest(), "byte_size": len(content)}


def reviewed_document(texts):
    source = reference("source_text", "\n".join(texts).encode())
    evidence = reference("review_evidence", b"Synthetic review declaration; no human review is asserted.\n")
    return {
        "schema": "aniflow.timed-text/v1", "source": source,
        "provenance": {
            "kind": "reviewed_lyrics",
            "authority": {"supplied_by": "synthetic-fixture-author", "provenance_artifact_id": "review_evidence"},
            "evidence": evidence, "source_sha256": source["sha256"],
        },
        "language": "en", "audio_source": None, "overlap_policy": "reject",
        "metadata": {"review_revision": "synthetic-r1"},
        "cues": [
            {"id": f"cue_{index + 1:06}", "source_label": None, "text": text,
             "timing": {"kind": "untimed"}, "speaker": None}
            for index, text in enumerate(texts)
        ],
    }


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def pcm(path):
    rate = 16000
    samples = b"".join(struct.pack("<h", 1200 if index % 64 < 32 else -1200) for index in range(rate * 2))
    with wave.open(str(path), "wb") as audio:
        audio.setparams((1, 2, rate, rate * 2, "NONE", "not compressed"))
        audio.writeframes(samples)


def exercise(binary, root, receipt):
    tools = root / "tools.json"
    write_json(tools, {
        "schema": "aniflow.audio-inspection.configuration/v1",
        "ffmpeg": local_tool_pin("ffmpeg"), "ffprobe": local_tool_pin("ffprobe"),
        "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536,
    })
    pocketsphinx = root / "synthetic-pocketsphinx"
    interpreter = Path(sys.executable).resolve(strict=True)
    pocketsphinx.write_text(f"#!{interpreter}\n" + POCKETSPHINX.replace("__FIXTURE_ROOT__", json.dumps(str(root))), encoding="utf-8")
    pocketsphinx.chmod(0o755)
    model = root / "synthetic-en-us"
    model.mkdir()
    resources = []
    for name in MODEL_FILES:
        path = model / name
        content = {"feat.params": FEATURE_PARAMETERS, "noisedict": NOISE_DICTIONARY}.get(
            name, f"Synthetic acoustic resource {name}; not inference weights.\n"
        )
        path.write_text(content, encoding="utf-8")
        resources.append({"name": name, "sha256": sha256(path), "byte_size": path.stat().st_size})
    dictionary = root / "synthetic-cmudict-en-us.dict"
    dictionary.write_text("hello HH AH L OW\nsynthetic S IH N TH EH T IH K\nworld W ER L D\n", encoding="utf-8")
    configuration = {
        "schema": "aniflow.audio-alignment.configuration/v1",
        "pocketsphinx": {"executable": str(pocketsphinx), "version": "5.1.1", "sha256": sha256(pocketsphinx)},
        "model": {"directory": str(model), "model_id": "en-us", "revision": "synthetic-fixture-v1", "files": resources},
        "dictionary": {"path": str(dictionary), "sha256": sha256(dictionary), "byte_size": dictionary.stat().st_size},
        "language": "en", "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536,
    }
    settings = root / "alignment.json"
    write_json(settings, configuration)
    source = root / "generated tone.wav"
    pcm(source)
    source_bytes = source.read_bytes()
    reviewed = reviewed_document(["  Hello, synthetic! ", "World!"])
    lyrics = root / "reviewed lyrics.json"
    write_json(lyrics, reviewed)
    lyrics_bytes = lyrics.read_bytes()
    (root / "mode").write_text("complete", encoding="utf-8")
    (root / "phrase").write_text("hello synthetic world", encoding="utf-8")
    shared = ["--input", source, "--configuration", tools, "--alignment-configuration", settings, "--lyrics", lyrics]
    cases = []
    documents = []

    plan = invoke(binary, "audio_plan", "audio", "plan", "--analysis", "lyrics-alignment", *shared)
    assert plan["payload"]["policy"]["offline"]
    assert "ai" in plan["payload"]["policy"]["allowed_side_effects"]
    assert "network" not in plan["payload"]["policy"]["allowed_side_effects"]
    assert not (root / "launches").exists()
    assert source.read_bytes() == source_bytes and lyrics.read_bytes() == lyrics_bytes
    cases.append("offline-plan-no-native-launch")

    def run(name, document=reviewed):
        current_lyrics_bytes = lyrics.read_bytes()
        outcome = invoke(binary, "audio_lyrics_align", "audio", "lyrics", "align", *shared, "--output-directory", root / name)
        outputs = {item["id"]: item for item in outcome["outputs"]}
        for item in outputs.values():
            path = Path(item["path"])
            assert sha256(path) == item["sha256"]
            documents.append((name + "-" + item["id"] + ".json", path))
        report = json.loads(Path(outputs["alignment"]["path"]).read_text())
        assert report["schema"] == "aniflow.audio-alignment/v1"
        assert report["result"]["status"] == "candidate"
        assert report["source"]["artifact"]["sha256"] == hashlib.sha256(source_bytes).hexdigest()
        assert report["confidence"]["kind"] == "unavailable"
        assert report["provenance"] == "probabilistic"
        assert report["reviewed_lyrics"]["document"] == document
        assert report["reviewed_lyrics"]["artifact"] == reference("reviewed_lyrics", current_lyrics_bytes)
        assert report["timed_text"]["provenance"] == document["provenance"]
        assert report["timed_text"]["metadata"] == {**document["metadata"], "aniflow_alignment_timing": "candidate"}
        assert not report["method"]["review_attestation_independently_verified"]
        assert [(cue["id"], cue["text"]) for cue in report["timed_text"]["cues"]] == [
            (cue["id"], cue["text"]) for cue in document["cues"]
        ]
        for word in report["result"]["observation"]["words"]:
            token = word["token"]
            cue = next(cue for cue in document["cues"] if cue["id"] == token["cue_id"])
            assert cue["text"].encode()[token["byte_start"]:token["byte_end"]].decode() == token["text"]
        assert source.read_bytes() == source_bytes and lyrics.read_bytes() == current_lyrics_bytes
        cases.append(name)
        return outcome, outputs, report

    outcome, outputs, report = run("complete-candidate")
    words = report["result"]["observation"]["words"]
    assert [word["timing"]["status"] for word in words] == ["candidate"] * 3
    assert words[0]["timing"] == {"status": "candidate", "start": {"numerator": 0, "denominator": 1}, "end": {"numerator": 2, "denominator": 5}}
    assert words[1]["timing"] == {"status": "candidate", "start": {"numerator": 1, "denominator": 2}, "end": {"numerator": 9, "denominator": 10}}
    assert words[2]["timing"] == {"status": "candidate", "start": {"numerator": 1, "denominator": 1}, "end": {"numerator": 7, "denominator": 5}}
    launches = (root / "launches").read_bytes()
    status = invoke(binary, "status_v3", "status-v3", outcome["run_directory"])
    assert status["payload"]["state"] == "complete"
    resumed = invoke(binary, "audio_resume", "audio", "resume", outcome["run_directory"], "--analysis", "lyrics-alignment", *shared)
    assert resumed["executed_stages"] == [] and resumed["reused_stages"] == outcome["executed_stages"]
    assert (root / "launches").read_bytes() == launches
    cases.append("status-exact-resume-no-relaunch")

    report_path = Path(outputs["alignment"]["path"])
    report_sha = sha256(report_path)
    export = root / "export"
    refused = invoke(binary, "audio_lyrics_export", "audio", "lyrics", "export", "--alignment", report_path,
                     "--to", "webvtt", "--output-directory", export, success=False)
    assert {loss["kind"] for loss in refused["losses"]} == {"language", "metadata"}
    assert not export.exists()
    cases.append("export-loss-refusal")
    exported = invoke(binary, "audio_lyrics_export", "audio", "lyrics", "export", "--alignment", report_path,
                      "--to", "webvtt", "--output-directory", export, "--allow-loss", "language,metadata")
    assert exported["alignment_report"]["sha256"] == report_sha
    assert exported["timing_authority"] == "candidate"
    conversion = json.loads((export / "conversion.json").read_text())
    assert conversion["output"]["sha256"] == sha256(export / "payload.vtt")
    assert conversion["input_document_sha256"] == sha256(export / "normalized-input.json")
    assert conversion["output_document_sha256"] == sha256(export / "normalized-output.json")
    assert not conversion["review_attestation_independently_verified"]
    assert json.loads((export / "normalized-output.json").read_text())["provenance"] == reviewed["provenance"]
    assert sha256(report_path) == report_sha
    assert source.read_bytes() == source_bytes and lyrics.read_bytes() == lyrics_bytes
    for name in ["normalized-input.json", "normalized-output.json", "conversion.json"]:
        documents.append(("export-" + name, export / name))
    cases.append("candidate-export-explicit-losses")

    (root / "mode").write_text("partial", encoding="utf-8")
    _, partial_outputs, partial = run("partial-unmatched-word")
    assert [word["timing"]["status"] for word in partial["result"]["observation"]["words"]] == ["candidate", "unmatched", "candidate"]
    assert partial["timed_text"]["cues"][0]["timing"] == {"kind": "untimed"}
    partial_export = root / "partial-export"
    invoke(binary, "audio_lyrics_export", "audio", "lyrics", "export", "--alignment", partial_outputs["alignment"]["path"],
           "--to", "webvtt", "--output-directory", partial_export, "--allow-loss", "language,metadata", success=False)
    assert not partial_export.exists()
    cases.append("partial-interval-format-refusal")

    repeated = reviewed_document(["Hello hello."])
    write_json(lyrics, repeated)
    (root / "phrase").write_text("hello hello", encoding="utf-8")
    (root / "mode").write_text("ambiguous", encoding="utf-8")
    _, _, ambiguous = run("repeated-word-ambiguity", repeated)
    assert [word["timing"]["status"] for word in ambiguous["result"]["observation"]["words"]] == ["ambiguous", "ambiguous"]
    assert ambiguous["timed_text"]["cues"][0]["timing"] == {"kind": "untimed"}
    lyrics.write_bytes(lyrics_bytes)
    (root / "phrase").write_text("hello synthetic world", encoding="utf-8")
    (root / "mode").write_text("complete", encoding="utf-8")

    launches = (root / "launches").read_bytes()
    changed = copy.deepcopy(reviewed)
    changed["metadata"]["review_revision"] = "synthetic-r2"
    write_json(lyrics, changed)
    invoke(binary, "audio_resume", "audio", "resume", outcome["run_directory"], "--analysis", "lyrics-alignment", *shared, success=False)
    assert (root / "launches").read_bytes() == launches
    lyrics.write_bytes(lyrics_bytes)
    cases.append("stale-review-revision-resume-refusal")

    resource = model / "means"
    resource_bytes = resource.read_bytes()
    resource.unlink()
    refused = invoke(binary, "audio_plan", "audio", "plan", "--analysis", "lyrics-alignment", *shared, success=False)
    assert any(item["code"] == "missing_model" for item in refused["diagnostics"])
    assert (root / "launches").read_bytes() == launches
    resource.write_bytes(resource_bytes)
    cases.append("missing-resource-preflight-refusal")

    unsupported = copy.deepcopy(configuration)
    unsupported["language"] = "fr"
    write_json(settings, unsupported)
    refused = invoke(binary, "audio_plan", "audio", "plan", "--analysis", "lyrics-alignment", *shared, success=False)
    assert any(item["code"] == "unsupported_language" for item in refused["diagnostics"])
    assert (root / "launches").read_bytes() == launches
    write_json(settings, configuration)
    cases.append("unsupported-language-preflight-refusal")
    assert source.read_bytes() == source_bytes and lyrics.read_bytes() == lyrics_bytes
    assert sha256(report_path) == report_sha

    result = {
        "schema": "aniflow.audio-alignment-smoke/v1", "synthetic_provider": True,
        "real_model_inference": False, "real_inspection_tools": True,
        "review_attestation_independently_verified": False,
        "cases": cases, "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
        "reviewed_lyrics_sha256": hashlib.sha256(lyrics_bytes).hexdigest(),
        "source_unchanged": True, "reviewed_lyrics_unchanged": True,
        "alignment_report_unchanged": True, "documents": [],
    }
    if receipt:
        receipt = receipt.resolve()
        receipt.parent.mkdir(parents=True, exist_ok=True)
        captured = Path(tempfile.mkdtemp(prefix="alignment-reports-", dir=receipt.parent))
        for name, path in documents:
            target = captured / name
            shutil.copyfile(path, target)
            result["documents"].append(str(target))
        write_json(receipt, result)
    print(json.dumps({"synthetic_alignment_cases": len(cases), "captured_documents": len(result["documents"])}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    arguments = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="aniflow alignment café ") as temporary:
        exercise(arguments.aniflow.resolve(strict=True), Path(temporary).resolve(), arguments.receipt)


if __name__ == "__main__":
    main()
