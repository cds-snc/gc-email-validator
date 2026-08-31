#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT

source_repository="https://github.com/cds-snc/gcorg-resolver"
official_dataset_url="https://open.canada.ca/data/en/dataset/57180b36-3428-4a7f-afe3-2161a6b44ec5"

github_curl_args=(--fail --location --show-error --silent)
if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  github_curl_args+=(--header "Authorization: Bearer ${GITHUB_TOKEN}")
fi

source_commit="$(
  curl "${github_curl_args[@]}" \
    --header "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/cds-snc/gcorg-resolver/commits/main" \
    | jq --exit-status --raw-output '.sha'
)"
raw_data_base="https://raw.githubusercontent.com/cds-snc/gcorg-resolver/${source_commit}/data"
concordance_url="${raw_data_base}/gc_concordance.csv"
org_info_url="${raw_data_base}/gc_org_info.csv"
aliases_url="${raw_data_base}/gc_org_aliases.csv"

curl "${github_curl_args[@]}" "${concordance_url}" --output "${temporary_dir}/gc_concordance.csv"
curl "${github_curl_args[@]}" "${org_info_url}" --output "${temporary_dir}/gc_org_info.csv"
curl "${github_curl_args[@]}" "${aliases_url}" --output "${temporary_dir}/gc_org_aliases.csv"

concordance_sha256="$(sha256sum "${temporary_dir}/gc_concordance.csv" | cut -d ' ' -f 1)"
org_info_sha256="$(sha256sum "${temporary_dir}/gc_org_info.csv" | cut -d ' ' -f 1)"
aliases_sha256="$(sha256sum "${temporary_dir}/gc_org_aliases.csv" | cut -d ' ' -f 1)"

jq --null-input \
  --arg sourceRepository "${source_repository}" \
  --arg sourceCommit "${source_commit}" \
  --arg officialDatasetUrl "${official_dataset_url}" \
  --arg concordanceUrl "${concordance_url}" \
  --arg orgInfoUrl "${org_info_url}" \
  --arg aliasesUrl "${aliases_url}" \
  --arg concordanceSha256 "${concordance_sha256}" \
  --arg orgInfoSha256 "${org_info_sha256}" \
  --arg aliasesSha256 "${aliases_sha256}" \
  '{
    sourceRepository: $sourceRepository,
    sourceCommit: $sourceCommit,
    officialDatasetUrl: $officialDatasetUrl,
    concordanceUrl: $concordanceUrl,
    orgInfoUrl: $orgInfoUrl,
    aliasesUrl: $aliasesUrl,
    concordanceSha256: $concordanceSha256,
    orgInfoSha256: $orgInfoSha256,
    aliasesSha256: $aliasesSha256
  }' > "${temporary_dir}/metadata.json"

cargo run --locked -p domain-compiler -- \
  --upstream-dir "${temporary_dir}" \
  --policy "${project_root}/data/domain-policy.yaml" \
  --output "${temporary_dir}/generated_domains.rs" \
  --manifest "${temporary_dir}/manifest.json"
rustfmt --edition 2024 "${temporary_dir}/generated_domains.rs"

install -m 0644 "${temporary_dir}/gc_concordance.csv" "${project_root}/data/upstream/gc_concordance.csv"
install -m 0644 "${temporary_dir}/gc_org_info.csv" "${project_root}/data/upstream/gc_org_info.csv"
install -m 0644 "${temporary_dir}/gc_org_aliases.csv" "${project_root}/data/upstream/gc_org_aliases.csv"
install -m 0644 "${temporary_dir}/metadata.json" "${project_root}/data/upstream/metadata.json"
install -m 0644 "${temporary_dir}/generated_domains.rs" "${project_root}/crates/api/src/generated_domains.rs"
install -m 0644 "${temporary_dir}/manifest.json" "${project_root}/data/manifest.json"

echo "Refreshed all domain data from gcorg-resolver commit ${source_commit}."
