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

# @description Process synthetic media through FFmpeg and a fixture provider.
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

    printf 'Smoke test passed: %s\n' "${final_video}"
    printf 'Delivery manifest: %s\n' "${delivery_manifest}"
    printf 'Provider lock: %s\n' "${provider_lock}"
    printf 'Provider report: %s\n' "${provider_report}"
}

aniflow_smoke_test "$@"
