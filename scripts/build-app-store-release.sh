#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
VERSION="$(node -p "require('${PROJECT_ROOT}/package.json').version")"
BUILD_USER_HOME="$(python3 -c 'from pathlib import Path; print(Path.home())')"
TARGET="${LINGZHAN_BUILD_TARGET:-universal-apple-darwin}"
DIST_DIR="${PROJECT_ROOT}/dist/app-store/${VERSION}"
APP_PATH="${DIST_DIR}/灵栈-${VERSION}-mac-app-store.app"
PKG_PATH="${DIST_DIR}/灵栈-${VERSION}-mac-app-store.pkg"
AUDIT_REPORT="${DIST_DIR}/audit/APP_STORE_BUNDLE_AUDIT-${VERSION}.json"
PROFILE_PATH="${PROJECT_ROOT}/src-tauri/profiles/LingStack_Mac_App_Store.provisionprofile"
RUSTUP_BIN_DIR="$(brew --prefix rustup 2>/dev/null)/bin"
BUILD_TARGET_DIR="$(mktemp -d "${TMPDIR%/}/lingzhan-store-build.XXXXXX")"

cleanup() {
  case "${BUILD_TARGET_DIR}" in
    "${TMPDIR%/}"/lingzhan-store-build.*) /bin/rm -rf -- "${BUILD_TARGET_DIR}" ;;
    *) echo "Refusing to clean unexpected build directory: ${BUILD_TARGET_DIR}" >&2 ;;
  esac
}
trap cleanup EXIT

if [[ ! -f "${PROFILE_PATH}" ]]; then
  echo "Missing Mac App Store provisioning profile: ${PROFILE_PATH}" >&2
  exit 1
fi

mkdir -p "${DIST_DIR}" "${DIST_DIR}/audit"
for artifact in "${APP_PATH}" "${PKG_PATH}"; do
  if [[ -e "${artifact}" ]]; then
    echo "Refusing to overwrite existing App Store artifact: ${artifact}" >&2
    exit 1
  fi
done

APP_SIGNING_IDENTITY="${LINGZHAN_APPLE_DISTRIBUTION_IDENTITY:-B225A892FF3723473D2F6205BD4E5E97FFBD5667}"
INSTALLER_IDENTITY="${LINGZHAN_INSTALLER_IDENTITY:-3rd Party Mac Developer Installer: zhu si (L4G2HAQ5B5)}"
if ! security find-identity -v -p codesigning | grep -Fq "${APP_SIGNING_IDENTITY}"; then
  echo "The Apple Distribution signing identity is unavailable." >&2
  exit 1
fi
if ! security find-certificate -a -c "${INSTALLER_IDENTITY}" -Z | grep -Fq 'SHA-1 hash:'; then
  echo "The Mac App Store installer identity is unavailable." >&2
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
  --features app-store \
  --config src-tauri/tauri.public.conf.json \
  --config src-tauri/tauri.app-store.conf.json \
  --config "{\"bundle\":{\"macOS\":{\"signingIdentity\":\"${APP_SIGNING_IDENTITY}\"}}}"

BUILT_APP="${BUILD_TARGET_DIR}/${TARGET}/release/bundle/macos/灵栈.app"
if [[ ! -d "${BUILT_APP}" ]]; then
  echo "Tauri did not produce the expected Mac App Store app." >&2
  exit 1
fi

ditto --norsrc "${BUILT_APP}" "${APP_PATH}"
codesign --verify --deep --strict --verbose=2 "${APP_PATH}"
# `:-` keeps plist output machine-readable on current macOS releases.
codesign -d --entitlements :- "${APP_PATH}" > "${DIST_DIR}/audit/signed-entitlements.plist"
/usr/libexec/PlistBuddy -c 'Print :com.apple.security.app-sandbox' "${DIST_DIR}/audit/signed-entitlements.plist" | grep -Fxq true
/usr/libexec/PlistBuddy -c 'Print :com.apple.application-identifier' "${DIST_DIR}/audit/signed-entitlements.plist" | grep -Fxq 'L4G2HAQ5B5.app.lingzhan.lingstack.store'
test -f "${APP_PATH}/Contents/embedded.provisionprofile"

python3 scripts/audit-clean-release.py "${APP_PATH}" \
  --expected-version "${VERSION}" \
  --expected-identifier app.lingzhan.lingstack.store \
  --allow-provisioning-profile \
  --forbid "$(id -un)" \
  --forbid "${PROJECT_ROOT}" \
  --report "${AUDIT_REPORT}"

xcrun productbuild \
  --sign "${INSTALLER_IDENTITY}" \
  --component "${APP_PATH}" /Applications \
  "${PKG_PATH}"
pkgutil --check-signature "${PKG_PATH}"
shasum -a 256 "${PKG_PATH}" "${APP_PATH}/Contents/MacOS/agent-skill-hub"

if [[ "${LINGZHAN_UPLOAD_APP_STORE:-0}" == "1" ]]; then
  : "${LINGZHAN_APP_STORE_KEY_ID:?Set LINGZHAN_APP_STORE_KEY_ID}"
  : "${LINGZHAN_APP_STORE_ISSUER:?Set LINGZHAN_APP_STORE_ISSUER}"
  : "${LINGZHAN_APP_STORE_P8:?Set LINGZHAN_APP_STORE_P8}"
  xcrun altool --validate-app -f "${PKG_PATH}" \
    --apiKey "${LINGZHAN_APP_STORE_KEY_ID}" \
    --apiIssuer "${LINGZHAN_APP_STORE_ISSUER}" \
    --p8-file-path "${LINGZHAN_APP_STORE_P8}"
  xcrun altool --upload-app -f "${PKG_PATH}" \
    --apiKey "${LINGZHAN_APP_STORE_KEY_ID}" \
    --apiIssuer "${LINGZHAN_APP_STORE_ISSUER}" \
    --p8-file-path "${LINGZHAN_APP_STORE_P8}"
fi
