#!/usr/bin/env bash

set -Eeuo pipefail

aniflow_smoke_test_directory=""

# @description Remove the temporary smoke-test workspace.
# @noargs
aniflow_cleanup_smoke_test() {
    if [[ -n "${aniflow_smoke_test_directory}" ]]; then
        rm -rf -- "${aniflow_smoke_test_directory}"
    fi
}
trap aniflow_cleanup_smoke_test EXIT

# @description Process synthetic media and exercise Pipeline v3 run-local resume.
# @noargs
# @exitcode 0 The synthetic end-to-end pipeline completed successfully.
# @exitcode 1 A dependency, build, or pipeline stage failed.
aniflow_smoke_test() {
    local repository_root
    repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

    aniflow_smoke_test_directory="$(mktemp -d)"

    local test_directory="${aniflow_smoke_test_directory}"

    printf 'Generating synthetic source video...\n'
    ffmpeg \
        -hide_banner \
        -loglevel error \
        -f lavfi \
        -i "testsrc2=size=320x180:rate=24:duration=2" \
        -f lavfi \
        -i "sine=frequency=440:sample_rate=48000:duration=2" \
        -c:v libx264 \
        -pix_fmt yuv420p \
        -c:a aac \
        -shortest \
        "${test_directory}/source.mp4"

    printf 'Building aniflow...\n'
    cargo build --manifest-path "${repository_root}/Cargo.toml"

    printf 'Running passthrough pipeline...\n'
    cargo run \
        --manifest-path "${repository_root}/Cargo.toml" \
        -- \
        run \
        --input "${test_directory}/source.mp4" \
        --pipeline "${repository_root}/pipelines/passthrough.yml" \
        --output-directory "${test_directory}/runs"

    local final_video
    final_video="$(
        find "${test_directory}/runs" \
            -type f \
            -path "*/output/master.mp4" \
            -print \
            -quit
    )"
    if [[ -z "${final_video}" ]]; then
        printf 'Smoke test did not produce a final video.\n' >&2
        return 1
    fi

    local delivery_manifest
    delivery_manifest="$(
        find "${test_directory}/runs" \
            -type f \
            -path "*/delivery/manifest.json" \
            -print \
            -quit
    )"
    if [[ -z "${delivery_manifest}" ]]; then
        printf 'Smoke test did not produce a delivery manifest.\n' >&2
        return 1
    fi

    ffprobe \
        -v error \
        -select_streams v:0 \
        -show_entries stream=width,height \
        -of default=noprint_wrappers=1 \
        "${final_video}"

    local copy_command
    copy_command="$(command -v cp)"
    local provider_pipeline="${test_directory}/provider.yml"
    printf '%s\n' \
        'version: 2' \
        'name: provider-smoke' \
        'frame_processors:' \
        '  - kind: external' \
        '    id: copy' \
        "    command: \"${copy_command}\"" \
        '    arguments: ["{input}", "{output}"]' \
        'renderflow:' \
        '  enabled: false' \
        'output:' \
        '  file: output/master.mp4' \
        > "${provider_pipeline}"

    printf 'Running fixture-provider pipeline...\n'
    cargo run \
        --manifest-path "${repository_root}/Cargo.toml" \
        -- \
        run \
        --input "${test_directory}/source.mp4" \
        --pipeline "${provider_pipeline}" \
        --output-directory "${test_directory}/provider-runs"

    local provider_lock
    provider_lock="$(
        find "${test_directory}/provider-runs" \
            -type f \
            -path "*/providers/frame_01_copy/provider-lock.json" \
            -print \
            -quit
    )"
    local provider_report
    provider_report="$(
        find "${test_directory}/provider-runs" \
            -type f \
            -path "*/providers/frame_01_copy/reports/frame-00000001-png.json" \
            -print \
            -quit
    )"
    if [[ -z "${provider_lock}" || -z "${provider_report}" ]]; then
        printf 'Smoke test did not retain provider lock and report evidence.\n' >&2
        return 1
    fi

    local v3_fixture_directory="${test_directory}/pipeline-v3-fixture"
    local v3_source_directory="${v3_fixture_directory}/source-frames"
    local v3_runs_directory="${v3_fixture_directory}/runs"
    mkdir -p -- "${v3_source_directory}"
    printf 'frame-one\n' > "${v3_source_directory}/frame-000001.txt"

    cp -- \
        "${repository_root}/docs/contracts/examples/provider-manifest-v1.example.json" \
        "${v3_fixture_directory}/provider-manifest-v1.example.json"
    cp -- \
        "${repository_root}/docs/contracts/examples/provider-configuration-v1.example.json" \
        "${v3_fixture_directory}/provider-configuration-v1.example.json"

    local v3_provider="${v3_fixture_directory}/fixture-provider.py"
    printf '%s\n' \
        '#!/usr/bin/env python3' \
        'import json' \
        'import pathlib' \
        'import shutil' \
        'import sys' \
        '' \
        'if len(sys.argv) != 3 or sys.argv[1] != "--aniflow-invocation":' \
        '    raise SystemExit(64)' \
        '' \
        'executable_directory = pathlib.Path(__file__).resolve().parent' \
        'counter_path = executable_directory / "launch-count"' \
        'launch_count = int(counter_path.read_text(encoding="utf-8")) if counter_path.exists() else 0' \
        'counter_path.write_text(str(launch_count + 1), encoding="utf-8")' \
        '' \
        'request_path = pathlib.Path(sys.argv[2])' \
        'request = json.loads(request_path.read_text(encoding="utf-8"))' \
        'source = pathlib.Path(request["inputs"][0]["path"])' \
        'destination = pathlib.Path(request["outputs"][0]["path"])' \
        'destination.parent.mkdir(parents=True, exist_ok=True)' \
        'shutil.copytree(source, destination)' \
        > "${v3_provider}"
    chmod 0755 -- "${v3_provider}"

    local v3_registration="${v3_fixture_directory}/provider-registration.json"
    printf '%s\n' \
        '{' \
        '  "schema": "aniflow.provider-registration/v1",' \
        '  "registration_id": "fixture-local",' \
        '  "manifest": "provider-manifest-v1.example.json",' \
        '  "configuration": "provider-configuration-v1.example.json",' \
        '  "executable": "fixture-provider.py",' \
        '  "implementation_id": "fixture-local-process",' \
        '  "components": {' \
        '    "tools": [' \
        '      {' \
        '        "id": "upscayl-bin",' \
        '        "version": "2.15.0"' \
        '      }' \
        '    ],' \
        '    "codecs": [' \
        '      {' \
        '        "id": "png",' \
        '        "version": "1.6.43"' \
        '      }' \
        '    ],' \
        '    "models": [' \
        '      {' \
        '        "id": "realesr-animevideov3",' \
        '        "version": "1.0.0",' \
        '        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"' \
        '      }' \
        '    ]' \
        '  }' \
        '}' \
        > "${v3_registration}"

    local v3_pipeline="${v3_fixture_directory}/pipeline.yml"
    printf '%s\n' \
        'schema: "aniflow.pipeline/v3"' \
        'name: "smoke-directory-copy"' \
        'inputs:' \
        '  - id: "source"' \
        '    artifact_type: "application/vnd.aniflow.frame-set+directory"' \
        '    artifact_role: "temporal_component"' \
        '    stream_role: "video"' \
        'stages:' \
        '  - id: "enhance"' \
        '    depends_on: []' \
        '    capability:' \
        '      id: "aniflow/frame.process"' \
        '      version_requirement: "^1.0"' \
        '    provider:' \
        '      primary:' \
        '        registration_id: "fixture-local"' \
        '      fallbacks: []' \
        '    inputs:' \
        '      - port: "frames"' \
        '        artifacts: ["source"]' \
        '    outputs:' \
        '      - port: "processed_frames"' \
        '        artifacts:' \
        '          - id: "enhanced"' \
        '            relative_path: "artifacts/enhance/frames"' \
        '            kind: "directory"' \
        '    validations:' \
        '      - id: "enhanced-integrity"' \
        '        artifact: "enhanced"' \
        '        contract: "aniflow.validation/artifact-integrity/v1"' \
        'outputs:' \
        '  - id: "enhanced-frames"' \
        '    artifact: "enhanced"' \
        '    required_validations: ["enhanced-integrity"]' \
        > "${v3_pipeline}"

    printf 'Running Pipeline v3 fixture provider...\n'
    cargo run \
        --manifest-path "${repository_root}/Cargo.toml" \
        -- \
        run-v3 \
        --pipeline "${v3_pipeline}" \
        --input "source=${v3_source_directory}" \
        --provider-registration "${v3_registration}" \
        --host-cpu-threads "8" \
        --host-memory-mib "16384" \
        --host-storage-mib "65536" \
        --host-gpu-available \
        --host-network-available \
        --allow-side-effect "filesystem-read" \
        --allow-side-effect "filesystem-write" \
        --allow-side-effect "subprocess" \
        --allow-side-effect "gpu" \
        --offline \
        --output-directory "${v3_runs_directory}" \
        --provider-timeout-seconds "60" \
        --provider-termination-grace-milliseconds "1000" \
        --maximum-stdout-bytes "1048576" \
        --maximum-stderr-bytes "1048576" \
        --maximum-artifact-files "100" \
        --maximum-artifact-bytes "10485760"

    local v3_run_directory
    v3_run_directory="$(
        find "${v3_runs_directory}" \
            -mindepth 1 \
            -maxdepth 1 \
            -type d \
            -print \
            -quit
    )"
    if [[ -z "${v3_run_directory}" ]]; then
        printf 'Pipeline v3 smoke test did not create a run workspace.\n' >&2
        return 1
    fi

    local v3_output="${v3_run_directory}/artifacts/enhance/frames/frame-000001.txt"
    local v3_checkpoint
    local v3_manifest
    v3_checkpoint="$(
        find "${v3_run_directory}/state/checkpoints" \
            -maxdepth 1 \
            -type f \
            -name '*.json' \
            -print \
            -quit
    )"
    v3_manifest="$(
        find "${v3_run_directory}/state/manifests" \
            -maxdepth 1 \
            -type f \
            -name '*.json' \
            -print \
            -quit
    )"
    if [[ ! -f "${v3_output}" || -z "${v3_checkpoint}" || -z "${v3_manifest}" ]]; then
        printf 'Pipeline v3 smoke test did not retain output and state evidence.\n' >&2
        return 1
    fi
    if ! cmp -- "${v3_source_directory}/frame-000001.txt" "${v3_output}"; then
        printf 'Pipeline v3 smoke-test output differs from its immutable input.\n' >&2
        return 1
    fi

    printf 'Resuming Pipeline v3 fixture provider...\n'
    cargo run \
        --manifest-path "${repository_root}/Cargo.toml" \
        -- \
        resume-v3 \
        "${v3_run_directory}" \
        --input "source=${v3_source_directory}" \
        --provider-registration "${v3_registration}" \
        --provider-timeout-seconds "60" \
        --provider-termination-grace-milliseconds "1000" \
        --maximum-stdout-bytes "1048576" \
        --maximum-stderr-bytes "1048576" \
        --maximum-artifact-files "100" \
        --maximum-artifact-bytes "10485760"

    local v3_counter="${v3_fixture_directory}/launch-count"
    if [[ ! -f "${v3_counter}" ]]; then
        printf 'Pipeline v3 smoke test did not retain its provider launch counter.\n' >&2
        return 1
    fi
    local v3_launch_count
    v3_launch_count="$(<"${v3_counter}")"
    if [[ "${v3_launch_count}" != "1" ]]; then
        printf 'Pipeline v3 unchanged resume relaunched the provider (%s launches).\n' \
            "${v3_launch_count}" >&2
        return 1
    fi

    printf 'Smoke test passed: %s\n' "${final_video}"
    printf 'Delivery manifest: %s\n' "${delivery_manifest}"
    printf 'Provider lock: %s\n' "${provider_lock}"
    printf 'Provider report: %s\n' "${provider_report}"
    printf 'Pipeline v3 workspace: %s\n' "${v3_run_directory}"
    printf 'Pipeline v3 checkpoint: %s\n' "${v3_checkpoint}"
}

aniflow_smoke_test "$@"
