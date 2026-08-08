#!/usr/bin/env bash

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
SCREENSHOT_DIR="${APP_STORE_SCREENSHOT_DIR:-${PROJECT_ROOT}/marketing/store-preview/output/zh-CN}"
EXCLUDED_SCREENSHOT="${APP_STORE_SCREENSHOT_EXCLUDE:-03-lingstack.png}"
FORBIDDEN_PATTERN='openai|chatgpt|openai_compatible|api\.openai\.com|gpt-'

if rg -n -i --glob '*.txt' "$FORBIDDEN_PATTERN" "${PROJECT_ROOT}/app-store/metadata"; then
  echo "App Store metadata contains a forbidden mainland-China reference." >&2
  exit 1
fi

if rg -n -i "$FORBIDDEN_PATTERN" \
  "${PROJECT_ROOT}/site/apple/lingstack" \
  "${PROJECT_ROOT}/marketing/store-preview/index.html" \
  "${PROJECT_ROOT}/marketing/generate-demo-home.mjs"; then
  echo "App Store marketing sources contain a forbidden mainland-China reference." >&2
  exit 1
fi

while IFS= read -r marketing_url; do
  if [[ -n "$(tr -d '[:space:]' < "$marketing_url")" ]]; then
    echo "Marketing URL must be empty for this submission: $marketing_url" >&2
    exit 1
  fi
done < <(find "${PROJECT_ROOT}/app-store/metadata" -name marketing_url.txt -type f -print)

if [[ ",${APP_STORE_SCREENSHOT_EXCLUDE:-03-lingstack.png}," != *",03-lingstack.png,"* ]]; then
  echo "The model-access promotional screenshot must remain excluded." >&2
  exit 1
fi

if ! command -v tesseract >/dev/null 2>&1; then
  echo "tesseract is required for the App Store screenshot compliance gate." >&2
  exit 1
fi

OCR_DIR="$(mktemp -d "${TMPDIR%/}/lingstack-store-ocr.XXXXXX")"
cleanup() {
  case "${OCR_DIR}" in
    "${TMPDIR%/}"/lingstack-store-ocr.*) rm -r -- "${OCR_DIR}" ;;
    *) echo "Refusing to clean unexpected OCR directory: ${OCR_DIR}" >&2 ;;
  esac
}
trap cleanup EXIT

selected_count=0
for screenshot in "${SCREENSHOT_DIR}"/*.png; do
  [[ -f "${screenshot}" ]] || continue
  screenshot_name="$(basename "${screenshot}")"
  [[ "${screenshot_name}" == "${EXCLUDED_SCREENSHOT}" ]] && continue
  tesseract "${screenshot}" "${OCR_DIR}/${screenshot_name%.png}" -l eng+chi_sim >/dev/null 2>&1
  selected_count=$((selected_count + 1))
done

if (( selected_count < 3 )); then
  echo "At least three compliant App Store screenshots are required by this release gate." >&2
  exit 1
fi

if rg -n -i "$FORBIDDEN_PATTERN" "${OCR_DIR}"; then
  echo "A selected App Store screenshot contains a forbidden mainland-China reference." >&2
  exit 1
fi

echo "App Store mainland-China compliance gate passed: 11 locales, ${selected_count} screenshots."
