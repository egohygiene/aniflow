#!/usr/bin/env bash

set -Eeuo pipefail

readonly product_name_pattern='aniflow|flow|optiflow|renderflow'

set +e
matches="$(
    git grep \
        --untracked \
        --exclude-standard \
        --line-number \
        --only-matching \
        --word-regexp \
        --ignore-case \
        --extended-regexp \
        -e "${product_name_pattern}" \
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

if [[ ${grep_status} -gt 1 ]]; then
    printf '%s\n' 'Unable to verify product naming.' >&2
    exit "${grep_status}"
fi

if [[ ${grep_status} -eq 0 ]]; then
    violations="$(
        printf '%s\n' "${matches}" \
            | awk -F ':' '$NF != tolower($NF)'
    )"

    if [[ -n "${violations}" ]]; then
        printf '%s\n' "${violations}" >&2
        printf '%s\n' \
            'flow-suite product names must remain lowercase.' \
            'Policy: docs/architecture/identity/PRINCIPLES.md#canonical-product-names' >&2
        exit 1
    fi
fi

printf '%s\n' 'Product naming is lowercase and consistent.'
