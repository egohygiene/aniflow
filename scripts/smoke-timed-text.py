#!/usr/bin/env python3
"""Exercise native timed-text conversion using generated, redistribution-safe text."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(binary, *arguments, success=True):
    result = subprocess.run(
        [str(binary), "--output", "json", "timed-text", *map(str, arguments)],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if (result.returncode == 0) != success:
        raise AssertionError(
            f"Unexpected exit {result.returncode}: {arguments}\n"
            f"stdout={result.stdout}\nstderr={result.stderr}"
        )
    return json.loads(result.stdout if success else result.stderr)


def run(binary, root, receipt):
    text = "Hello café 雪 🌍\nمرحبا שלום\n"
    fixtures = {
        "plain": text,
        "lrc": "[00:00.25]Hello café 雪 🌍\n[00:02.00]مرحبا שלום\n",
        "srt": "1\n00:00:00,250 --> 00:00:01,500\nHello café 雪 🌍\n\n2\n00:00:02,000 --> 00:00:03,750\nمرحبا שלום\n",
        "webvtt": "WEBVTT\n\n00:00:00.250 --> 00:00:01.500\nHello café 雪 🌍\n\n00:00:02.000 --> 00:00:03.750\nمرحبا שלום\n",
        "ttml": '<tt xmlns="http://www.w3.org/ns/ttml" xml:space="preserve"><body><div><p begin="00:00:00.250" end="00:00:01.500">Hello café 雪 🌍</p><p begin="00:00:02.000" end="00:00:03.750">مرحبا שלום</p></div></body></tt>',
    }
    registry = invoke(binary, "formats")
    assert registry["command"] == "timed_text_formats"
    for name in [*fixtures, "json"]:
        assert f'"{name}"' in json.dumps(registry["result"])
    sources = {}
    reports = []
    cases = []

    def convert(name, source, from_format, to_format, *extra, success=True):
        output = root / name
        original_sha256 = sha256(source)
        envelope = invoke(
            binary, "convert", "--input", source, "--from", from_format,
            "--to", to_format, "--output-directory", output, *extra, success=success,
        )
        assert sha256(source) == original_sha256
        assert envelope["command"] == "timed_text_convert"
        if success:
            report = json.loads((output / "conversion.json").read_text())
            assert report["schema"] == "aniflow.timed-text-conversion/v1"
            assert report["input"]["sha256"] == original_sha256
            payloads = list(output.glob("payload.*"))
            assert len(payloads) == 1
            assert report["output"]["sha256"] == sha256(payloads[0])
            assert report["input_document_sha256"] == sha256(output / "normalized-input.json")
            assert report["output_document_sha256"] == sha256(output / "normalized-output.json")
            assert not report["review_attestation_independently_verified"]
            assert sorted(path.name for path in output.iterdir()) == sorted([
                payloads[0].name, "normalized-input.json", "normalized-output.json", "conversion.json",
            ])
            reports.append(output)
        else:
            assert not output.exists(), f"refused conversion created {output}"
        cases.append({"id": name, "outcome": "accepted" if success else "refused"})
        return output, envelope

    for name, content in fixtures.items():
        source = root / (name + ".source")
        source.write_bytes(content.encode("utf-8"))
        sources[source] = sha256(source)
        transport, _ = convert(name + "-json", source, name, "json")
        canonical = json.loads((transport / "normalized-input.json").read_text())
        if name == "plain":
            assert len(canonical["cues"]) == 1 and canonical["cues"][0]["text"] == text
        else:
            assert canonical["cues"][0]["text"] == "Hello café 雪 🌍"
            assert canonical["cues"][1]["text"] == "مرحبا שלום"
        assert canonical["provenance"]["kind"] == "unreviewed"
        roundtrip, _ = convert(name + "-roundtrip", transport / "payload.json", "json", name)
        assert json.loads((roundtrip / "normalized-output.json").read_text())["cues"] == canonical["cues"]

    srt = root / "srt.source"
    convert("srt-webvtt", srt, "srt", "webvtt")
    _, refusal = convert("unapproved-end-loss", srt, "srt", "lrc", success=False)
    assert any(loss["kind"] == "end_times" for loss in refusal["result"]["losses"])
    approved, _ = convert("approved-end-loss", srt, "srt", "lrc", "--allow-loss", "end_times,cue_identifiers")
    assert any(loss["kind"] == "end_times" for loss in json.loads((approved / "conversion.json").read_text())["losses"])
    convert("never-invent-timing", root / "plain.source", "plain", "srt", success=False)

    bom = root / "bom.srt"
    bom.write_bytes(b"\xef\xbb\xbf" + fixtures["srt"].replace("\n", "\r\n").encode())
    sources[bom] = sha256(bom)
    lexical, _ = convert("lexical-facts", bom, "srt", "json")
    facts = json.loads((lexical / "conversion.json").read_text())["lexical"]
    assert facts["utf8_bom_removed"] and facts["crlf_pairs"] > 0 and facts["lone_cr"] == 0

    reviewed = root / "reviewed-context.json"
    reviewed.write_text(json.dumps({
        "schema": "aniflow.timed-text-context/v1",
        "provenance": {"kind": "reviewed_lyrics", "authority": {
            "supplied_by": "synthetic-reviewer", "provenance_artifact_id": "supplied_review",
        }, "evidence": {"id": "supplied_review", "sha256": hashlib.sha256(b"synthetic supplied review").hexdigest(), "byte_size": 25},
            "source_sha256": sha256(srt)},
        "language": None, "audio_source": None, "overlap_policy": "reject",
    }))
    reviewed_output, _ = convert("reviewed-preserved", srt, "srt", "json", "--context", reviewed)
    assert json.loads((reviewed_output / "normalized-output.json").read_text())["provenance"]["kind"] == "reviewed_lyrics"

    unsafe_xml = root / "external-entity.ttml"
    unsafe_xml.write_text('<!DOCTYPE tt [<!ENTITY private SYSTEM "file:///unread-synthetic-path">]><tt xmlns="http://www.w3.org/ns/ttml" xml:space="preserve"><body><div><p begin="00:00:00.000" end="00:00:01.000">&private;</p></div></body></tt>')
    sources[unsafe_xml] = sha256(unsafe_xml)
    convert("xml-entity-refusal", unsafe_xml, "ttml", "json", success=False)
    before = {path.name: sha256(path) for path in approved.iterdir()}
    refused = invoke(binary, "convert", "--input", srt, "--from", "srt", "--to", "lrc", "--output-directory", approved, "--allow-loss", "end_times,cue_identifiers", success=False)
    assert refused["status"] == "error"
    assert before == {path.name: sha256(path) for path in approved.iterdir()}
    cases.append({"id": "existing-output-refusal", "outcome": "refused"})
    for source, expected in sources.items():
        assert sha256(source) == expected

    result = {"schema": "aniflow.timed-text-smoke/v1", "synthetic_only": True,
              "cases": cases, "source_sha256_preserved": True, "documents": []}
    if receipt:
        receipt = receipt.resolve()
        receipt.parent.mkdir(parents=True, exist_ok=True)
        captured = Path(tempfile.mkdtemp(prefix="timed-text-reports-", dir=receipt.parent))
        registry_path = captured / "registry.json"
        registry_path.write_text(json.dumps(registry["result"], ensure_ascii=False) + "\n")
        context_path = captured / "reviewed-context.json"
        shutil.copyfile(reviewed, context_path)
        result["registry"] = str(registry_path)
        result["context"] = str(context_path)
        for output in reports:
            destination = captured / output.name
            shutil.copytree(output, destination)
            result["documents"].extend(str(destination / name) for name in ["normalized-input.json", "normalized-output.json", "conversion.json"])
        receipt.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"synthetic_timed_text_cases": len(cases), "captured_documents": len(result["documents"])}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    arguments = parser.parse_args()
    binary = arguments.aniflow.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="aniflow timed text café ") as temporary:
        root = Path(temporary)
        run(binary, root, arguments.receipt)


if __name__ == "__main__":
    main()
