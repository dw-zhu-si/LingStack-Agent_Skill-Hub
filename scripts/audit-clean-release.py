#!/usr/bin/env python3
"""Fail a Lingzhan public artifact if it contains private Agent/Skill data."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import re
import stat
import sys
import zipfile
from pathlib import Path, PurePosixPath


TEXT_SECRET_PATTERNS = (
    re.compile(rb"\bsk-[A-Za-z0-9_-]{16,}\b"),
    re.compile(rb"\bmh_[A-Za-z0-9_-]{16,}\b"),
    re.compile(rb"(?i)\b(api[_-]?key|access[_-]?token|secret)\s*[:=]\s*[A-Za-z0-9_./+-]{16,}"),
    re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
)
FORBIDDEN_DEFINITION_SUFFIXES = (".md", ".toml", ".yaml", ".yml", ".json")
EXPECTED_FILES = {
    "Contents/Info.plist",
    "Contents/MacOS/agent-skill-hub",
    "Contents/Resources/icon.icns",
    "Contents/_CodeSignature/CodeResources",
}
# A notarized `.app` contains Apple's stapled ticket at this optional path.
OPTIONAL_FILES = {"Contents/CodeResources"}
MAX_ZIP_MEMBER_BYTES = 128 * 1024 * 1024


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("artifact", type=Path)
    parser.add_argument("--registry", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--forbid", action="append", default=[])
    parser.add_argument("--forbid-text", action="append", default=[])
    parser.add_argument("--expected-version", default="0.11.0")
    parser.add_argument("--expected-identifier", default="app.lingzhan.agent-skill-hub")
    parser.add_argument("--allow-provisioning-profile", action="store_true")
    return parser.parse_args()


def app_payload(app: Path) -> tuple[dict[str, bytes], bytes]:
    if not app.is_dir() or app.suffix != ".app":
        raise ValueError("artifact is not a macOS .app directory")
    files: dict[str, bytes] = {}
    for path in sorted(app.rglob("*")):
        if path.is_symlink():
            raise ValueError("release bundle contains a symlink")
        if path.is_file():
            relative = path.relative_to(app).as_posix()
            files[relative] = path.read_bytes()
    return files, files.get("Contents/Info.plist", b"")


def zip_payload(archive: Path) -> tuple[dict[str, bytes], bytes]:
    files: dict[str, bytes] = {}
    plist = b""
    with zipfile.ZipFile(archive) as bundle:
        roots = set()
        for info in bundle.infolist():
            path = PurePosixPath(info.filename)
            if path.is_absolute() or ".." in path.parts:
                raise ValueError("release ZIP contains an unsafe path")
            if path.parts:
                roots.add(path.parts[0])
            mode = info.external_attr >> 16
            if stat.S_ISLNK(mode):
                raise ValueError("release ZIP contains a symlink")
            if info.is_dir():
                continue
            if info.file_size > MAX_ZIP_MEMBER_BYTES:
                raise ValueError("release ZIP member exceeds the size limit")
            if len(path.parts) < 2:
                raise ValueError("release ZIP contains a file outside the app root")
            relative = PurePosixPath(*path.parts[1:]).as_posix()
            data = bundle.read(info)
            files[relative] = data
            if relative == "Contents/Info.plist":
                plist = data
        if len(roots) != 1 or not next(iter(roots)).endswith(".app"):
            raise ValueError("release ZIP must contain exactly one macOS app root")
    return files, plist


def registry_markers(path: Path | None) -> tuple[list[bytes], int]:
    if path is None or not path.is_file():
        return [], 0
    registry = json.loads(path.read_text(encoding="utf-8"))
    markers: set[bytes] = set()
    groups = registry.get("groups", [])
    for group in groups:
        logical_id = str(group.get("logical_id", ""))
        if len(logical_id) >= 12:
            markers.add(logical_id.encode())
        for variant in group.get("variants", []):
            digest = str(variant.get("sha256", ""))
            if len(digest) == 64:
                markers.add(digest.encode())
            for location in variant.get("locations", []):
                source = str(location.get("path", ""))
                if len(source) >= 12:
                    markers.add(source.encode())
    return sorted(markers), len(groups)


def audit(args: argparse.Namespace) -> dict[str, object]:
    artifact = args.artifact.resolve()
    if artifact.suffix == ".zip":
        files, plist_bytes = zip_payload(artifact)
        artifact_type = "zip"
    else:
        files, plist_bytes = app_payload(artifact)
        artifact_type = "app"

    file_names = set(files)
    expected_files = set(EXPECTED_FILES)
    if args.allow_provisioning_profile:
        expected_files.add("Contents/embedded.provisionprofile")
    unexpected_files = sorted(file_names - expected_files - OPTIONAL_FILES)
    missing_files = sorted(expected_files - file_names)
    definition_files = sorted(
        name for name in file_names if name.lower().endswith(FORBIDDEN_DEFINITION_SUFFIXES)
    )
    combined = b"\n".join(files.values())
    forbidden = [b"/Users/", b"/Volumes/", b"/private/var/folders/"]
    forbidden.extend(value.encode() for value in args.forbid if value)
    personal_path_matches = sum(1 for marker in forbidden if marker in combined)
    secret_matches = sum(len(pattern.findall(combined)) for pattern in TEXT_SECRET_PATTERNS)
    lowered_payload = combined.lower()
    forbidden_text_matches = {
        value: lowered_payload.count(value.lower().encode()) for value in args.forbid_text
    }
    forbidden_text_match_count = sum(forbidden_text_matches.values())
    markers, registry_group_count = registry_markers(args.registry)
    personal_registry_matches = sum(1 for marker in markers if marker in combined)

    plist = plistlib.loads(plist_bytes)
    identifier = plist.get("CFBundleIdentifier", "")
    version = plist.get("CFBundleShortVersionString", "")
    errors = []
    if identifier != args.expected_identifier:
        errors.append("bundle identifier does not match the expected release identifier")
    if version != args.expected_version:
        errors.append("bundle version does not match the expected release version")
    if unexpected_files:
        errors.append("bundle contains files outside the release allowlist")
    if missing_files:
        errors.append("bundle is missing required files")
    if definition_files:
        errors.append("bundle contains Agent/Skill-shaped definition files")
    if personal_path_matches:
        errors.append("bundle contains a personal absolute path marker")
    if personal_registry_matches:
        errors.append("bundle contains identifiers, hashes, or paths from the private registry")
    if secret_matches:
        errors.append("bundle contains a secret-shaped value")
    if forbidden_text_match_count:
        errors.append("bundle contains a forbidden public-distribution reference")

    result: dict[str, object] = {
        "schema_version": 1,
        "status": "passed" if not errors else "failed",
        "artifact_type": artifact_type,
        "artifact_sha256": hashlib.sha256(artifact.read_bytes()).hexdigest()
        if artifact.is_file()
        else "",
        "bundle_identifier": identifier,
        "bundle_version": version,
        "bundle_file_count": len(files),
        "bundled_agent_skill_definition_count": len(definition_files),
        "private_registry_reference_count": registry_group_count,
        "private_registry_match_count": personal_registry_matches,
        "personal_path_match_count": personal_path_matches,
        "secret_shape_match_count": secret_matches,
        "forbidden_text_match_count": forbidden_text_match_count,
        "forbidden_text_matches": forbidden_text_matches,
        "unexpected_file_count": len(unexpected_files),
        "missing_file_count": len(missing_files),
        "errors": errors,
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return result


def main() -> int:
    args = parse_args()
    try:
        result = audit(args)
    except Exception as error:  # Fail closed and avoid printing artifact contents.
        print(json.dumps({"status": "failed", "error": str(error)}, ensure_ascii=False))
        return 1
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
