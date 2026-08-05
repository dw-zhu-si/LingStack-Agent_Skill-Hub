#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
VERSION="$(node -p "require('${PROJECT_ROOT}/package.json').version")"
BUILD_USER_HOME="$(python3 -c 'from pathlib import Path; print(Path.home())')"
TARGET="${LINGZHAN_BUILD_TARGET:-universal-apple-darwin}"
ARCH_LABEL="${TARGET/universal-apple-darwin/universal}"
ARCH_LABEL="${ARCH_LABEL/aarch64-apple-darwin/arm64}"
ARCH_LABEL="${ARCH_LABEL/x86_64-apple-darwin/x86_64}"
DIST_DIR="${PROJECT_ROOT}/dist/public-release/${VERSION}"
APP_PATH="${DIST_DIR}/灵栈-${VERSION}-public-clean-macos-${ARCH_LABEL}.app"
ZIP_PATH="${DIST_DIR}/灵栈-${VERSION}-public-clean-macos-${ARCH_LABEL}.zip"
DMG_PATH="${DIST_DIR}/灵栈-${VERSION}-public-clean-macos-${ARCH_LABEL}.dmg"
AUDIT_DIR="${DIST_DIR}/audit"
APP_REPORT="${AUDIT_DIR}/CLEAN_RELEASE_APP_AUDIT-${VERSION}.json"
ZIP_REPORT="${AUDIT_DIR}/CLEAN_RELEASE_ZIP_AUDIT-${VERSION}.json"
DMG_REPORT="${AUDIT_DIR}/CLEAN_RELEASE_DMG_AUDIT-${VERSION}.json"
RUSTUP_BIN_DIR="$(brew --prefix rustup 2>/dev/null)/bin"
BUILD_TARGET_DIR="$(mktemp -d "${TMPDIR%/}/lingzhan-public-build.XXXXXX")"
MOUNT_POINT=""

cleanup() {
  if [[ -n "${MOUNT_POINT}" ]] && mount | grep -Fq " on ${MOUNT_POINT} "; then
    hdiutil detach "${MOUNT_POINT}" -quiet || true
  fi
  case "${BUILD_TARGET_DIR}" in
    "${TMPDIR%/}"/lingzhan-public-build.*) /bin/rm -rf -- "${BUILD_TARGET_DIR}" ;;
    *) echo "Refusing to clean unexpected build directory: ${BUILD_TARGET_DIR}" >&2 ;;
  esac
}
trap cleanup EXIT

mkdir -p "${DIST_DIR}" "${AUDIT_DIR}"

for artifact in "${APP_PATH}" "${ZIP_PATH}" "${DMG_PATH}"; do
  if [[ -e "${artifact}" ]]; then
    echo "Refusing to overwrite existing release: ${artifact}" >&2
    exit 1
  fi
done

SIGNING_IDENTITY="${LINGZHAN_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning | awk '/Developer ID Application:/{print $2; exit}')}"
if [[ -z "${SIGNING_IDENTITY}" ]]; then
  echo "A valid Developer ID Application identity is required for the public release." >&2
  exit 1
fi

export PATH="${RUSTUP_BIN_DIR}:${PATH}"
export CARGO_TARGET_DIR="${BUILD_TARGET_DIR}"
export VITE_LINGZHAN_PUBLIC_RELEASE=1
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=${BUILD_USER_HOME}=/__build_home__"$'\x1f'"--remap-path-prefix=${PROJECT_ROOT}=/__source__"

cd "${PROJECT_ROOT}"
npm run tauri build -- \
  --target "${TARGET}" \
  --bundles app \
  --features public-release \
  --config src-tauri/tauri.public.conf.json \
  --config "{\"bundle\":{\"macOS\":{\"signingIdentity\":\"${SIGNING_IDENTITY}\",\"hardenedRuntime\":true}}}"

BUNDLE_ROOT="${BUILD_TARGET_DIR}/${TARGET}/release/bundle"
BUILT_APP="${BUNDLE_ROOT}/macos/灵栈.app"
if [[ ! -d "${BUILT_APP}" ]]; then
  echo "Tauri did not produce the expected app artifact." >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "${BUILT_APP}"
ditto --norsrc "${BUILT_APP}" "${APP_PATH}"
codesign --verify --deep --strict --verbose=2 "${APP_PATH}"

NOTARY_KEY="${LINGZHAN_NOTARY_KEY:-}"
NOTARY_KEY_ID="${LINGZHAN_NOTARY_KEY_ID:-}"
NOTARY_ISSUER="${LINGZHAN_NOTARY_ISSUER:-}"
if [[ -n "${NOTARY_KEY}" || -n "${NOTARY_KEY_ID}" || -n "${NOTARY_ISSUER}" ]]; then
  : "${NOTARY_KEY:?Set LINGZHAN_NOTARY_KEY}"
  : "${NOTARY_KEY_ID:?Set LINGZHAN_NOTARY_KEY_ID}"
  : "${NOTARY_ISSUER:?Set LINGZHAN_NOTARY_ISSUER}"
  NOTARY_UPLOAD="${BUILD_TARGET_DIR}/notary-upload.zip"
  ditto -c -k --norsrc --keepParent "${APP_PATH}" "${NOTARY_UPLOAD}"
  xcrun notarytool submit "${NOTARY_UPLOAD}" --wait \
    --key "${NOTARY_KEY}" --key-id "${NOTARY_KEY_ID}" --issuer "${NOTARY_ISSUER}"
  xcrun stapler staple "${APP_PATH}"
  xcrun stapler validate "${APP_PATH}"
fi

DMG_STAGE="${BUILD_TARGET_DIR}/dmg-stage"
mkdir -p "${DMG_STAGE}"
ditto --norsrc "${APP_PATH}" "${DMG_STAGE}/灵栈.app"
ln -s /Applications "${DMG_STAGE}/Applications"
hdiutil create -volname "灵栈" -srcfolder "${DMG_STAGE}" -format UDZO "${DMG_PATH}"
codesign --force --timestamp --sign "${SIGNING_IDENTITY}" "${DMG_PATH}"
codesign --verify --verbose=2 "${DMG_PATH}"
hdiutil verify "${DMG_PATH}"

if [[ -n "${NOTARY_KEY}" ]]; then
  xcrun notarytool submit "${DMG_PATH}" --wait \
    --key "${NOTARY_KEY}" --key-id "${NOTARY_KEY_ID}" --issuer "${NOTARY_ISSUER}"
  xcrun stapler staple "${DMG_PATH}"
  xcrun stapler validate "${DMG_PATH}"
fi

AUDIT_ARGS=(--expected-version "${VERSION}" --forbid "$(id -un)" --forbid "${PROJECT_ROOT}")
python3 scripts/audit-clean-release.py "${APP_PATH}" "${AUDIT_ARGS[@]}" --report "${APP_REPORT}"

ditto -c -k --norsrc --keepParent "${APP_PATH}" "${ZIP_PATH}"
unzip -tq "${ZIP_PATH}"
python3 scripts/audit-clean-release.py "${ZIP_PATH}" "${AUDIT_ARGS[@]}" --report "${ZIP_REPORT}"

MOUNT_POINT="$(mktemp -d "${TMPDIR%/}/lingzhan-dmg-audit.XXXXXX")"
hdiutil attach -readonly -nobrowse -mountpoint "${MOUNT_POINT}" "${DMG_PATH}" -quiet
python3 scripts/audit-clean-release.py "${MOUNT_POINT}/灵栈.app" "${AUDIT_ARGS[@]}" --report "${DMG_REPORT}"
hdiutil detach "${MOUNT_POINT}" -quiet
rmdir "${MOUNT_POINT}"
MOUNT_POINT=""

file "${APP_PATH}/Contents/MacOS/agent-skill-hub"
shasum -a 256 "${ZIP_PATH}" "${DMG_PATH}" "${APP_PATH}/Contents/MacOS/agent-skill-hub"
