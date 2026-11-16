#!/usr/bin/env python3
"""Observe existing alignment cancellation/recovery with model-free fixtures.

Linux /proc identities make stopped, zombie and absent observations explicit.
This observer does not change process ownership, set a subreaper, or qualify
PocketSphinx inference, real lyrics, native platforms or human review.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import struct
import subprocess
import time


TREE = r'''
if mode.startswith("tree_"):
    import signal, subprocess
    def event(kind, **value):
        value.update(event=kind, pid=os.getpid(), ppid=os.getppid(), pgid=os.getpgrp())
        with (root / "tree-events.jsonl").open("a") as stream:
            stream.write(json.dumps(value)+"\n")
    child_script = r"""
import json, os, pathlib, signal, subprocess, sys, time
root = pathlib.Path(sys.argv[1])
def event(kind, **value):
    value.update(event=kind, pid=os.getpid(), ppid=os.getppid(), pgid=os.getpgrp())
    with (root / "tree-events.jsonl").open("a") as stream:
        stream.write(json.dumps(value)+"\n")
grand_script = ''' + repr('''
import json, os, pathlib, signal, sys, time
root = pathlib.Path(sys.argv[1])
def event(kind, **value):
    value.update(event=kind, pid=os.getpid(), ppid=os.getppid(), pgid=os.getpgrp())
    with (root / "tree-events.jsonl").open("a") as stream:
        stream.write(json.dumps(value)+"\\n")
def stop(signum, _frame):
    event("grandchild_term", signal=signum)
    raise SystemExit(0)
signal.signal(signal.SIGTERM, stop)
event("grandchild_started")
deadline = time.monotonic()+20
while time.monotonic() < deadline: time.sleep(0.01)
event("grandchild_guard_expired")
''') + r'''
grand = subprocess.Popen([sys.executable, "-c", grand_script, str(root)])
def stop(signum, _frame):
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    try: grand.terminate()
    except ProcessLookupError: pass
    status = grand.wait(timeout=2)
    event("child_waited_grandchild", child_pid=grand.pid, wait_status=status, signal=signum)
    raise SystemExit(0)
signal.signal(signal.SIGTERM, stop)
event("child_started", child_pid=grand.pid)
deadline = time.monotonic()+20
while time.monotonic() < deadline: time.sleep(0.01)
stop(0, None)
"""
    child = subprocess.Popen([sys.executable, "-c", child_script, str(root)])
    def stop(signum, _frame):
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        try: child.terminate()
        except ProcessLookupError: pass
        status = child.wait(timeout=2)
        event("aligner_waited_child", child_pid=child.pid, wait_status=status, signal=signum)
        raise SystemExit(0)
    signal.signal(signal.SIGTERM, stop)
    event("aligner_started", child_pid=child.pid)
    deadline = time.monotonic()+5
    while not (root / "tree-events.jsonl").read_text().count("grandchild_started"):
        if time.monotonic() >= deadline: raise RuntimeError("tree fixture did not start")
        time.sleep(0.01)
    if mode == "tree_flood":
        print("x"*100000, flush=True)
    deadline = time.monotonic()+20
    while time.monotonic() < deadline: time.sleep(0.01)
    stop(0, None)
'''


def pin(path: Path):
    raw = path.read_bytes()
    return {"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}


def write_json(path: Path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False)+"\n")


def proc(pid: int):
    try:
        raw = Path(f"/proc/{pid}/stat").read_text()
    except FileNotFoundError:
        return {"pid": pid, "state": "absent"}
    tail = raw[raw.rindex(")")+2:].split()
    return {"pid": pid, "state": tail[0], "ppid": int(tail[1]),
            "pgid": int(tail[2]), "start_ticks": int(tail[19])}


def fixture(source: Path, root: Path, mode: str, timeout_ms: int):
    root.mkdir()
    text = (source/"tests/audio_alignment.rs").read_text()
    def raw(name):
        return re.search(r"const "+name+r': &str = r#"(.*?)"#;', text, re.S)[1]
    def escaped(name):
        value = re.search(r"const "+name+r': &str = "((?:[^"\\]|\\.)*)";', text, re.S)[1]
        return json.loads('"'+value+'"')
    config = {"schema": "aniflow.audio-inspection.configuration/v1",
              "tool_timeout_milliseconds": 5000, "maximum_tool_output_bytes": 65536}
    for name in ["ffmpeg", "ffprobe"]:
        p = root/name
        p.write_text(raw("INSPECTION_TOOL"))
        p.chmod(0o755)
        config[name] = {"executable": str(p), "version": "6.1.1", "sha256": pin(p)["sha256"]}
    aligner = root/"synthetic-pocketsphinx"
    code = raw("SYNTHETIC_ALIGNER").replace("__FIXTURE_ROOT__", json.dumps(str(root)))
    code = code.replace('if mode == "sleep": time.sleep(120)', TREE+'\nif mode == "sleep": time.sleep(120)')
    aligner.write_text(code)
    aligner.chmod(0o755)
    model = root/"model"
    model.mkdir()
    files = []
    for name in ["feat.params", "mdef", "means", "noisedict", "sendump", "transition_matrices", "variances"]:
        data = (escaped("FEATURE_PARAMS").encode() if name == "feat.params" else
                escaped("NOISE_DICTIONARY").encode() if name == "noisedict" else
                b"Synthetic model identity; not inference weights.\n")
        (model/name).write_bytes(data)
        files.append({"name": name, "sha256": pin(model/name)["sha256"], "byte_size": len(data)})
    (model/"untracked-resource").write_bytes(b"must remain untouched")
    dictionary = root/"dictionary.dict"
    dictionary.write_bytes(b"hello HH AH L OW\nsynthetic S IH N TH EH T IH K\nworld W ER L D\n")
    settings = {"schema": "aniflow.audio-alignment.configuration/v1",
                "pocketsphinx": {"executable": str(aligner), "version": "5.1.1", "sha256": pin(aligner)["sha256"]},
                "model": {"directory": str(model), "model_id": "en-us", "revision": "synthetic-fixture-v1", "files": files},
                "dictionary": {"path": str(dictionary), "sha256": pin(dictionary)["sha256"], "byte_size": dictionary.stat().st_size},
                "language": "en", "tool_timeout_milliseconds": timeout_ms, "maximum_tool_output_bytes": 65536}
    pcm = b"".join(struct.pack("<h", 1200 if n % 64 < 32 else -1200) for n in range(32000))
    (root/"source.wav").write_bytes(b"RIFF"+struct.pack("<I", len(pcm)+36)+b"WAVEfmt "+struct.pack("<IHHIIHH",16,1,1,16000,32000,2,16)+b"data"+struct.pack("<I",len(pcm))+pcm)
    lyrics = json.loads((source/"docs/contracts/examples/timed-text-v1.example.json").read_text())
    lyrics.update(language="en", audio_source=None, metadata={"revision": "review-r1"})
    lyrics["cues"][0]["text"] = "Hello, synthetic!"
    lyrics["cues"][1]["text"] = "World."
    for cue in lyrics["cues"]:
        cue["timing"] = {"kind": "untimed"}
    for name, value in [("tools.json",config),("settings.json",settings),("reviewed.json",lyrics)]:
        write_json(root/name, value)
    (root/"mode").write_text(mode)
    protected = {str(p.relative_to(root)):pin(p) for p in root.rglob("*") if p.is_file() and p.name != "mode"}
    return protected


def envelope(run, success: bool):
    assert (run.returncode == 0) == success, (run.returncode, run.stdout, run.stderr)
    value = json.loads(run.stdout if success else run.stderr)
    assert value["schema_version"] == 1 and value["status"] == ("success" if success else "error")
    return value


def current_manifest(run: Path):
    file = sorted((run/"state/manifests").glob("*.json"))[-1]
    return json.loads(file.read_text())


def events(root: Path):
    file = root/"tree-events.jsonl"
    if not file.exists():
        return []
    return [json.loads(line) for line in file.read_text().splitlines()]


def run_case(binary: Path, source: Path, work: Path, name: str, mode: str,
             expected: str, cancel=False, outer_timeout=False):
    root = work/name
    protected = fixture(source, root, mode, 10_000 if cancel or outer_timeout else 1000)
    base = ["--input",str(root/"source.wav"),"--configuration",str(root/"tools.json"),
            "--alignment-configuration",str(root/"settings.json"),"--lyrics",str(root/"reviewed.json")]
    common = ["--maximum-artifact-files","24","--maximum-artifact-bytes",str(512*1024*1024)]
    if outer_timeout:
        common += ["--provider-timeout-seconds","1"]
    commands = []
    def capture(label, args, success):
        start = time.monotonic()
        r = subprocess.run([str(binary),"--output","json",*args],capture_output=True,timeout=20)
        (root/(label+".stdout")).write_bytes(r.stdout)
        (root/(label+".stderr")).write_bytes(r.stderr)
        commands.append({"id":label,"native_exit":r.returncode,"elapsed_seconds":round(time.monotonic()-start,6),
                         "stdout":pin(root/(label+".stdout")),"stderr":pin(root/(label+".stderr"))})
        return envelope(r,success)
    args = ["audio","lyrics","align",*base,"--output-directory",str(root/"runs"),*common]
    started = time.monotonic()
    with (root/"run.stdout").open("wb") as out, (root/"run.stderr").open("wb") as err:
        child = subprocess.Popen([str(binary),"--output","json",*args],stdout=out,stderr=err)
        before = {}
        try:
            deadline = time.monotonic()+10
            while time.monotonic() < deadline:
                rows = events(root)
                found = {row["event"]: row for row in rows if row["event"].endswith("_started")}
                if len(found) == 3:
                    before = {key:proc(row["pid"]) for key,row in found.items()}
                    aligner = found["aligner_started"]
                    before["provider"] = proc(aligner["ppid"])
                    group = before["provider"]["pid"]
                    assert before["provider"]["ppid"] == child.pid
                    assert all(p["state"] not in ["absent","Z"] and p["pgid"] == group for p in before.values())
                    assert before["child_started"]["ppid"] == before["aligner_started"]["pid"]
                    assert before["grandchild_started"]["ppid"] == before["child_started"]["pid"]
                    break
                if child.poll() is not None:
                    raise AssertionError("CLI returned before full fixture tree was observed")
                time.sleep(0.005)
            assert before, "fixture descendants were not observed before cancellation/bound"
            if cancel:
                child.send_signal(signal.SIGINT)
            code = child.wait(timeout=15)
        except BaseException:
            if child.poll() is None:
                child.send_signal(signal.SIGINT)
                try:
                    child.wait(timeout=4)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
            raise
    assert code == 7, code
    error = json.loads((root/"run.stderr").read_text())
    assert error["status"] == "error" and error["error"]["category"] == "execution"
    commands.append({"id":"run","native_exit":code,"elapsed_seconds":round(time.monotonic()-started,6),
                     "stdout":pin(root/"run.stdout"),"stderr":pin(root/"run.stderr")})
    run = Path(error["result"]["run_directory"])
    manifest = current_manifest(run)
    assert manifest["payload"]["state"] == expected, manifest
    stages = manifest["payload"]["stages"]
    assert [s["state"] for s in stages] == ["complete", expected]
    checkpoint = next((run/"state/checkpoints").glob("inspect_audio-*.json"))
    checkpoint_pin = pin(checkpoint)
    assert not list((run/"state/checkpoints").glob("analyze_alignment-*.json"))
    assert not (run/"artifacts/audio-alignment/alignment.json").exists()
    reports = sorted((run/"providers").glob("analyze_alignment-*.report.json"))
    report = json.loads(reports[-1].read_text())
    expected_outcome = "cancelled" if cancel else "timed_out" if outer_timeout else "failed"
    assert report["payload"]["outcome"] == expected_outcome, report["payload"]
    if not cancel and not outer_timeout:
        text = report["payload"]["stderr"]["retained_text"]
        diagnostic = "ToolOutputLimit" if mode == "tree_flood" else "ToolTimeout"
        assert diagnostic in text, text
    after = {role:proc(p["pid"]) for role,p in before.items()}
    assert after["provider"]["state"] == "absent", after
    for role,p in after.items():
        assert p["state"] in ["absent","Z"], (role,p)
        if p["state"] != "absent":
            assert p["start_ticks"] == before[role]["start_ticks"], "PID identity was reused"
    tree = events(root)
    waited = [e for e in tree if e["event"] in ["aligner_waited_child","child_waited_grandchild"]]
    for event in waited:
        assert event["wait_status"] == 0
    status = capture("status_after_stop",["status-v3",str(run)],True)
    assert status["result"]["payload"]["state"] == expected
    (root/"mode").write_text("ok")
    original_checkpoint = checkpoint_pin
    launches_before = (root/"launches").read_text().splitlines()
    resume = capture("resume",["audio","resume",str(run),"--analysis","lyrics-alignment",*base,*common],True)
    assert resume["result"]["reused_stages"] == ["inspect_audio"]
    assert resume["result"]["executed_stages"] == ["analyze_alignment"]
    assert pin(checkpoint) == original_checkpoint
    assert len((root/"launches").read_text().splitlines()) == len(launches_before)+1
    assert current_manifest(run)["payload"]["state"] == "complete"
    alignment = run/"artifacts/audio-alignment/alignment.json"
    accepted = pin(alignment)
    again = capture("complete_resume",["audio","resume",str(run),"--analysis","lyrics-alignment",*base,*common],True)
    assert again["result"]["reused_stages"] == ["inspect_audio","analyze_alignment"]
    assert again["result"]["executed_stages"] == []
    assert pin(alignment) == accepted and pin(checkpoint) == original_checkpoint
    assert len((root/"launches").read_text().splitlines()) == len(launches_before)+1
    for relative, value in protected.items():
        assert pin(root/relative) == value, relative
    return {"case":name,"state":"passed","initial_run_state":expected,"outer_outcome":expected_outcome,
            "stopped_manifest_sha256":manifest["manifest_sha256"],
            "stopped_provider_report":{"file":str(reports[-1].relative_to(root)),**pin(reports[-1]),
                                       "termination":report["payload"]["termination"],
                                       "failure":report["payload"]["failure"],
                                       "stderr":report["payload"]["stderr"]},
            "observed_before":before,"observed_after":after,"fixture_wait_evidence":waited,
            "all_selected_processes_not_running":True,"all_selected_descendants_reaped":all(p["state"]=="absent" for p in after.values()),
            "inspect_checkpoint":checkpoint_pin,"accepted_alignment":accepted,"protected_originals_unchanged":True,
            "commands":commands,"compatible_resume_reused_inspection_only":True,"complete_resume_reused_all":True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--work-directory", type=Path, required=True)
    parser.add_argument("--source-revision", required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    work = args.work_directory
    assert work.is_absolute() and not work.exists() and work.parent.is_dir()
    assert re.fullmatch("[0-9a-f]{40}",args.source_revision)
    work.mkdir(mode=0o700)
    source = Path(__file__).resolve().parent.parent
    subjects = {"binary":pin(binary),"observer":pin(Path(__file__)),"fixture_source":pin(source/"tests/audio_alignment.rs")}
    cases = [run_case(binary,source,work,"cancel","tree_sleep","cancelled",cancel=True),
             run_case(binary,source,work,"inner-timeout","tree_sleep","failed"),
             run_case(binary,source,work,"inner-output-bound","tree_flood","failed"),
             run_case(binary,source,work,"outer-timeout","tree_sleep","failed",outer_timeout=True)]
    assert subjects == {"binary":pin(binary),"observer":pin(Path(__file__)),"fixture_source":pin(source/"tests/audio_alignment.rs")}
    files = {str(p.relative_to(work)):pin(p) for p in work.rglob("*") if p.is_file()}
    report = {"schema_version":"aniflow.alignment-recovery-observation/v1","state":"passed",
              "measured_source_revision":args.source_revision,"subjects":subjects,"cases":cases,"retained_files":files,
              "historical_missing_marker_cause_established":False,"native_alignment_model_qualified":False,
              "global_subreaper_changed":False,"release_or_human_approval":False,
              "scope":"Linux synthetic trusted tools and inherited process group; stopped/zombie/absent and fixture-owned waits are distinct. No universal descendant reaping or escaped-group containment claim."}
    write_json(work/"report.json",report)
    print(json.dumps({"state":"alignment_recovery_checks_passed","cases":len(cases),
                      "commands":sum(len(c["commands"]) for c in cases),"report":pin(work/"report.json")},sort_keys=True))


if __name__ == "__main__":
    main()
