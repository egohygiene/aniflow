#!/usr/bin/env python3
"""Exercise stem lineage from genuine synthetic separation runs; never run models."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import runpy
import struct
import subprocess
import sys
import tempfile
import wave


ROOT = Path(__file__).resolve().parents[1]
INSPECTION = runpy.run_path(str(Path(__file__).with_name("smoke-audio-inspection.py")))
sha256 = INSPECTION["sha256"]
run_json = INSPECTION["run_json"]

MODEL = r'''#!/usr/bin/python3
import json, pathlib, struct, sys, wave
args = sys.argv[1:]
if args[:3] == ["-I", "-B", "-c"]:
    print(json.dumps({"demucs_version": "4.0.1", "python_version": "3.11.0"}))
    raise SystemExit(0)
assert args[:4] == ["-I", "-B", "-m", "demucs.separate"], args
source = pathlib.Path(args[-1])
target = pathlib.Path(args[args.index("--out") + 1]) / "htdemucs_6s" / source.stem
target.mkdir(parents=True)
with wave.open(str(source), "rb") as audio:
    frames = audio.getnframes() * 44100 // audio.getframerate()
for name, amplitude in [("vocals", 1200), ("no_vocals", 600)]:
    with wave.open(str(target / (name + ".wav")), "wb") as audio:
        audio.setparams((2, 2, 44100, frames, "NONE", "not compressed"))
        audio.writeframes(struct.pack("<hh", amplitude, -amplitude) * frames)
'''

SEPARATOR = r'''#!/usr/bin/python3
import json, pathlib, struct, sys, wave
assert len(sys.argv) == 3 and sys.argv[1] == "--aniflow-invocation"
request = json.loads(pathlib.Path(sys.argv[2]).read_text())
settings = request["configuration"]["values"]
for binding in request["outputs"]:
    target = pathlib.Path(binding["path"])
    target.parent.mkdir(parents=True, exist_ok=True)
    if binding["port"] == "evidence":
        target.write_bytes(b"opaque synthetic separator evidence; not Demucs JSON\x00\xff")
        continue
    rate = settings["rate"]
    frames = rate // 10 + settings["extra_frames"]
    amplitude = 1200 if binding["port"] == "percussion" else 600
    with wave.open(str(target), "wb") as audio:
        audio.setparams((2, 2, rate, frames, "NONE", "not compressed"))
        audio.writeframes(struct.pack("<hh", amplitude, -amplitude) * frames)
'''

TOOLS = r'''#!/usr/bin/python3
import hashlib, json, pathlib, sys, time, wave
root = pathlib.Path(__file__).parent
name = pathlib.Path(__file__).name
args = sys.argv[1:]
if args == ["-version"]:
    print(name + " version 6.1.1 synthetic")
    raise SystemExit(0)
source = pathlib.Path(args[args.index("-i") + 1])
with wave.open(str(source), "rb") as audio:
    rate, channels, frames = audio.getframerate(), audio.getnchannels(), audio.getnframes()
    pcm = audio.readframes(frames)
with (root / "launches").open("a") as log:
    log.write(json.dumps(args) + "\n")
if name == "ffprobe":
    print(json.dumps({"streams": [{"index": 0, "codec_type": "audio", "codec_name": "pcm_s16le", "sample_fmt": "s16",
        "sample_rate": str(rate), "channels": channels, "bits_per_sample": 16, "time_base": "1/" + str(rate),
        "duration_ts": frames, "bit_rate": str(rate * channels * 16)}], "format": {"format_name": "wav", "nb_streams": 1}}))
    raise SystemExit(0)
(root / "analysis-started").write_text("yes")
if (root / "mode").read_text() == "sleep":
    time.sleep(120)
if not any("ebur128=" in arg for arg in args):
    print("SHA256=" + hashlib.sha256(pcm).hexdigest())
    raise SystemExit(0)
if not any("apad=" in arg for arg in args):
    for index in range(frames // (rate // 10)):
        pts = index * (rate // 10)
        print(f"frame:{index} pts:{pts} pts_time:{pts / rate}")
        print("lavfi.r128.S=-28.700")
print("""[Parsed_ebur128_0] Summary:

  Integrated loudness:
    I: -28.7 LUFS
    Threshold: -38.7 LUFS

  Loudness range:
    LRA: 0.0 LU
    Threshold: -48.7 LUFS
    LRA low: -28.7 LUFS
    LRA high: -28.7 LUFS

  True peak:
    Peak: -28.7 dBFS
""", file=sys.stderr)
'''


def write_json(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def executable(path: Path, body: str) -> Path:
    path.write_text(body, encoding="utf-8")
    path.chmod(0o755)
    return path


def canonical_digest(value: dict) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def generic_bundle(directory: Path, rate: int, extra_frames: int) -> tuple[str, list[str]]:
    directory.mkdir()
    manifest = json.loads((ROOT / "providers/demucs/manifest.json").read_text())
    identity = {"id": "org.example.synthetic.stem-separator", "version": "1.0.0"}
    manifest["provider"] = {**identity, "display_name": "Synthetic provider-neutral separator"}
    schema_bytes = b'{"type":"object","additionalProperties":false,"properties":{"rate":{"type":"integer"},"extra_frames":{"type":"integer"}},"required":["rate","extra_frames"]}\n'
    (directory / "configuration.schema.json").write_bytes(schema_bytes)
    schema = {"id": "aniflow.synthetic-stems/v1", "version": "1.0.0", "sha256": hashlib.sha256(schema_bytes).hexdigest()}
    manifest["configuration_schemas"] = [schema]
    capability = manifest["capabilities"][0]
    capability["configuration_schema"] = schema
    capability["requirements"]["tools"] = []
    capability["requirements"]["models"] = []
    for port, name in zip(capability["outputs"], ["percussion", "ambience", "evidence"]):
        port["name"] = name
    capability["outputs"][2]["artifact_type"] = "application/octet-stream"
    values = {"rate": rate, "extra_frames": extra_frames}
    configuration = {"schema": "aniflow.provider-configuration/v1", "provider": identity,
        "capability": {"id": "aniflow/audio.separate", "version": "1.0.0"},
        "configuration_schema": schema, "values": values, "effective_configuration_sha256": canonical_digest(values)}
    write_json(directory / "manifest.json", manifest)
    write_json(directory / "configuration.json", configuration)
    executable(directory / "provider.py", SEPARATOR)
    write_json(directory / "registration.json", {"schema": "aniflow.provider-registration/v1", "registration_id": "synthetic-stems",
        "manifest": "manifest.json", "configuration": "configuration.json", "executable": "provider.py",
        "implementation_id": "synthetic-stems-v1", "components": {"tools": [], "codecs": [], "models": []}})
    ports = [("percussion", "drums", "wav"), ("ambience", "room_tone", "wav"), ("evidence", "opaque_evidence", "bin")]
    pipeline = {"schema": "aniflow.pipeline/v3", "name": "synthetic-stem-lineage",
        "inputs": [{"id": "source_audio", "artifact_type": "audio/wav", "artifact_role": "temporal_component", "stream_role": "audio"}],
        "stages": [{"id": "separate_synthetic", "depends_on": [], "capability": {"id": "aniflow/audio.separate", "version_requirement": "=1.0.0"},
            "provider": {"primary": {"registration_id": "synthetic-stems"}, "fallbacks": []}, "inputs": [{"port": "audio", "artifacts": ["source_audio"]}],
            "outputs": [{"port": port, "artifacts": [{"id": artifact, "relative_path": f"artifacts/separate/{artifact}.{suffix}", "kind": "file"}]} for port, artifact, suffix in ports],
            "validations": [{"id": artifact + "_integrity", "artifact": artifact, "contract": "aniflow.validation/artifact-integrity/v1"} for _, artifact, _ in ports]}],
        "outputs": [{"id": artifact, "artifact": artifact, "required_validations": [artifact + "_integrity"]} for _, artifact, _ in ports]}
    write_json(directory / "pipeline.yml", pipeline)
    return "separate_synthetic", ["drums", "room_tone"]


def prepare_fixture(binary: Path, directory: Path, provider: str, *, synthetic_tools: bool,
                    rate: int = 48000, extra_frames: int = 0) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    mix = directory / "synthetic mix café 雪.wav"
    with wave.open(str(mix), "wb") as audio:
        audio.setparams((2, 2, 48000, 4800, "NONE", "not compressed"))
        audio.writeframes(struct.pack("<hh", 1600, -1600) * 4800)
    bundle = directory / "provider bundle"
    if provider == "demucs":
        model_tools = directory / "fake model tools"
        model_tools.mkdir()
        model = executable(model_tools / "python", MODEL)
        for name in ["ffmpeg", "ffprobe"]:
            executable(model_tools / name, f'#!/usr/bin/python3\nprint("{name} version 6.1.1 synthetic")\n')
        cache = directory / "synthetic checkpoint assets"
        cache.mkdir()
        (cache / "htdemucs_6s.yaml").write_text("models: ['5c90dfd2']\n")
        (cache / "5c90dfd2-34c22ccb.th").write_bytes(b"synthetic bytes; never load with PyTorch")
        subprocess.run([sys.executable, str(ROOT / "providers/demucs/workflow.py"), "prepare", "--python", str(model),
            "--ffmpeg", str(model_tools / "ffmpeg"), "--ffprobe", str(model_tools / "ffprobe"),
            "--model-repository", str(cache), "--output-directory", str(bundle)], check=True, capture_output=True, timeout=30)
        stage, stems = "separate_vocals", ["vocals", "accompaniment"]
    else:
        stage, stems = generic_bundle(bundle, rate, extra_frames)
    arguments = ["run-v3", "--pipeline", str(bundle / "pipeline.yml"), "--input", "source_audio=" + str(mix),
        "--provider-registration", str(bundle / "registration.json"), "--output-directory", str(directory / "separation runs"),
        "--host-cpu-threads", "2", "--host-memory-mib", "16384", "--host-storage-mib", "16384", "--offline"]
    for effect in ["filesystem-read", "filesystem-write", "environment-read", "subprocess", "ai"]:
        arguments += ["--allow-side-effect", effect]
    separation = run_json(binary, "run_v3", *arguments)
    tools = directory / "inspection tools"
    tools.mkdir()
    (tools / "mode").write_text("ok")
    if synthetic_tools:
        pins = {name: {"executable": str(executable(tools / name, TOOLS)), "version": "6.1.1",
                       "sha256": hashlib.sha256(TOOLS.encode()).hexdigest()} for name in ["ffmpeg", "ffprobe"]}
    else:
        pins = {name: INSPECTION["local_tool_pin"](name) for name in ["ffmpeg", "ffprobe"]}
    configuration = directory / "tool-pins.json"
    write_json(configuration, {"schema": "aniflow.audio-inspection.configuration/v1", **pins,
        "tool_timeout_milliseconds": 30000 if not synthetic_tools else 5000, "maximum_tool_output_bytes": 65536})
    descriptor = {"mix": str(mix), "mix_sha256": sha256(mix), "separation_run": separation["run_directory"],
        "stage": stage, "stems": stems, "outputs": separation["outputs"], "configuration": str(configuration), "tools": str(tools)}
    write_json(directory / "fixture.json", descriptor)
    return descriptor


def exercise(binary: Path, directory: Path, provider: str, reports: Path | None) -> dict:
    fixture = prepare_fixture(binary, directory, provider, synthetic_tools=False)
    before = {output["artifact"]: sha256(Path(output["path"])) for output in fixture["outputs"]}
    cases = []
    for stem in fixture["stems"]:
        shared = ["--input", fixture["mix"], "--configuration", fixture["configuration"],
            "--stem-run", fixture["separation_run"], "--stem-stage", fixture["stage"], "--stem-id", stem]
        run_json(binary, "audio_plan", "audio", "plan", *shared)
        outcome = run_json(binary, "audio_inspect", "audio", "inspect", *shared, "--output-directory", str(directory / ("inspect-" + stem)))
        outputs = {item["id"]: item for item in outcome["outputs"]}
        analysis = json.loads(Path(outputs["analysis"]["path"]).read_text())
        assert analysis["source"]["stem"]["id"] == stem, analysis["source"]
        assert "stem_lineage" in outputs
        lineage = json.loads(Path(outputs["stem_lineage"]["path"]).read_text())
        assert lineage["lineage"]["original_mix"]["artifact"]["sha256"] == fixture["mix_sha256"]
        assert lineage["lineage"]["selected_stem"]["artifact"]["sha256"] == analysis["source"]["artifact"]["sha256"]
        assert lineage["lineage"]["timing_basis"] == "zero_origin_duration_only"
        status = run_json(binary, "status_v3", "status-v3", outcome["run_directory"])
        assert status["payload"]["state"] == "complete"
        resumed = run_json(binary, "audio_resume", "audio", "resume", outcome["run_directory"], *shared)
        assert resumed["executed_stages"] == []
        assert resumed["outputs"] == outcome["outputs"]
        case = {"stem_id": stem, "stem_sha256": analysis["source"]["artifact"]["sha256"],
            "sample_rate_hz": analysis["source"]["sample_rate_hz"], "frame_count": analysis["source"]["frame_count"],
            "analysis_sha256": outputs["analysis"]["sha256"], "lineage_sha256": outputs["stem_lineage"]["sha256"],
            "reused_stages": resumed["reused_stages"]}
        if reports is not None:
            for key in ["analysis", "stem_lineage"]:
                name = f"{provider}-{stem}-{key}.json"
                (reports / name).write_bytes(Path(outputs[key]["path"]).read_bytes())
                case[key + "_report"] = str(reports / name)
        cases.append(case)
    assert sha256(Path(fixture["mix"])) == fixture["mix_sha256"]
    assert {output["artifact"]: sha256(Path(output["path"])) for output in fixture["outputs"]} == before
    print(f"Stem lineage smoke passed: {provider}, {', '.join(fixture['stems'])}; synthetic separation, real local inspection")
    return {"provider": provider, "mix_sha256": fixture["mix_sha256"], "source_and_separation_unchanged": True,
        "separation_inference": "synthetic_fake_only", "cases": cases}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--prepare-only", type=Path)
    parser.add_argument("--provider", choices=["demucs", "generic"], default="generic")
    parser.add_argument("--rate", type=int, default=48000)
    parser.add_argument("--extra-frames", type=int, default=0)
    parser.add_argument("--receipt", type=Path)
    arguments = parser.parse_args()
    binary = arguments.aniflow.resolve(strict=True)
    if arguments.prepare_only:
        descriptor = prepare_fixture(binary, arguments.prepare_only.resolve(), arguments.provider,
            synthetic_tools=True, rate=arguments.rate, extra_frames=arguments.extra_frames)
        print(json.dumps(descriptor))
        return
    reports = None
    if arguments.receipt:
        arguments.receipt.parent.mkdir(parents=True, exist_ok=True)
        reports = arguments.receipt.resolve().with_suffix("").with_name(arguments.receipt.stem + "-reports")
        reports.mkdir(exist_ok=True)
    results = []
    with tempfile.TemporaryDirectory(prefix="aniflow-stem-smoke-") as temporary:
        root = Path(temporary).resolve()
        for provider in ["demucs", "generic"]:
            results.append(exercise(binary, root / provider, provider, reports))
    if arguments.receipt:
        write_json(arguments.receipt, {"schema": "aniflow.audio-stem-smoke/v1", "generated_at": datetime.now(timezone.utc).isoformat(),
            "aniflow_sha256": sha256(binary), "tools": {name: INSPECTION["local_tool_pin"](name) for name in ["ffmpeg", "ffprobe"]},
            "fixtures": results, "unverified": ["real separation inference", "model quality", "native macOS", "release qualification"]})


if __name__ == "__main__":
    main()
