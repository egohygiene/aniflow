#!/usr/bin/env python3
"""Read-only host/storage facts for the existing Upscayl qualification workflow.

This does not install tools, launch models, test devices, create media, run a
scheduler or accept outputs. Invoke aniflow's existing plan/run/resume commands
separately. A Vulkan loader or device node alone is not provider qualification.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import stat


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--storage-parent", required=True, type=Path)
    args = parser.parse_args()
    parent = args.storage_parent
    if not parent.is_absolute() or parent.resolve(strict=True) != parent or not parent.is_dir():
        parser.error("storage parent must be an existing canonical absolute directory")
    device_root = Path("/dev/dri")
    devices = []
    if device_root.is_dir():
        for path in sorted(device_root.iterdir()):
            if path.name.startswith(("card", "renderD")):
                info = path.lstat()
                devices.append({"name": path.name, "character_device": stat.S_ISCHR(info.st_mode),
                                "readable": os.access(path, os.R_OK), "writable": os.access(path, os.W_OK)})
    icds = []
    for directory in [Path("/etc/vulkan/icd.d"), Path("/usr/share/vulkan/icd.d")]:
        if not directory.is_dir():
            continue
        for path in sorted(directory.glob("*.json")):
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 262144:
                icds.append({"name": path.name, "read_status": "unsupported_or_over_limit"})
                continue
            data = path.read_bytes()
            try:
                document = json.loads(data)
                icds.append({"name": path.name, "sha256": hashlib.sha256(data).hexdigest(),
                             "library_path_declaration": document.get("ICD", {}).get("library_path"),
                             "read_status": "observed_not_executed"})
            except (ValueError, AttributeError):
                icds.append({"name": path.name, "read_status": "invalid_json"})
    usage = shutil.disk_usage(parent)
    print(json.dumps({
        "schema": "aniflow.upscayl-resource-host-observation/v1",
        "operating_system": platform.system(), "architecture": platform.machine(),
        "kernel_release": platform.release(), "storage_parent": str(parent),
        "filesystem_bytes": {"total": usage.total, "used": usage.used, "free": usage.free},
        "linux_dri_nodes": devices, "vulkan_icd_documents": icds,
        "provider_invoked": False, "device_or_backend_tested": False,
        "native_qualified": False,
        "interpretation": "read-only host facts, not a capability probe; no device nodes/ICDs observed does not establish every possible backend; Mac/native GPU/model, memory and media acceptance remain separately required"
    }, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
