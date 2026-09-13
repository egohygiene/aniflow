#!/usr/bin/env bash

set -Eeuo pipefail

readonly script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly repository_root="$(cd "${script_directory}/.." && pwd)"
readonly checker="${script_directory}/check-product-names.sh"
readonly cases_file="${repository_root}/tests/fixtures/product-names/cases.tsv"
readonly temporary_directory="$(mktemp -d)"
readonly fixture_file="${temporary_directory}/product-names.md"

# @description Remove the isolated fixture repository.
cleanup() {
    rm -rf -- "${temporary_directory}"
}

trap cleanup EXIT

git -C "${temporary_directory}" init --quiet

tab="$(printf '\t')"
while IFS="${tab}" read -r canonical title_case uppercase mixed_case; do
    if [[ -z "${canonical}" || "${canonical}" == \#* ]]; then
        continue
    fi

    printf '# %s guide\n%s remains independent.\n' \
        "${canonical}" \
        "${canonical}" >"${fixture_file}"

    if ! output="$(cd "${temporary_directory}" && "${checker}" 2>&1)"; then
        printf 'Expected canonical spelling to pass: %s\n%s\n' \
            "${canonical}" \
            "${output}" >&2
        exit 1
    fi

    for rejected in "${title_case}" "${uppercase}" "${mixed_case}"; do
        printf '# %s guide\n' "${rejected}" >"${fixture_file}"

        set +e
        output="$(cd "${temporary_directory}" && "${checker}" 2>&1)"
        checker_status=$?
        set -e

        if [[ ${checker_status} -eq 0 ]]; then
            printf 'Expected noncanonical spelling to fail: %s\n' \
                "${rejected}" >&2
            exit 1
        fi

        if [[ "${output}" != *"${rejected}"* ]]; then
            printf 'Expected diagnostic to identify spelling: %s\n%s\n' \
                "${rejected}" \
                "${output}" >&2
            exit 1
        fi
    done
done <"${cases_file}"

printf '%s\n' 'Product-name policy fixtures passed.'
