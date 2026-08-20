#!/usr/bin/env bash

set -Eeuo pipefail

set +e
violations="$(
    git grep \
        --untracked \
        --exclude-standard \
        --line-number \
        --word-regexp \
        --extended-regexp \
        -e 'Aniflow|ANIFLOW|Optiflow|OPTIFLOW|Renderflow|RENDERFLOW|Flow|FLOW' \
        -- \
        '*.json' \
        '*.md' \
        '*.rs' \
        '*.toml' \
        '*.yaml' \
        '*.yml'
)"
grep_status=$?
set -e

if [[ ${grep_status} -eq 0 ]]; then
    printf '%s\n' "${violations}" >&2
    printf '%s\n' \
        'Product names must remain lowercase: aniflow, flow, optiflow, renderflow.' >&2
    exit 1
fi

if [[ ${grep_status} -ne 1 ]]; then
    printf '%s\n' 'Unable to verify product naming.' >&2
    exit "${grep_status}"
fi

printf '%s\n' 'Product naming is lowercase and consistent.'
