#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HTML_PATH="$SCRIPT_DIR/store-preview/index.html"
OUTPUT_DIR="$SCRIPT_DIR/store-preview/output/zh-CN"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

if [[ ! -x "$CHROME" ]]; then
  echo "Google Chrome is required to render App Store previews." >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
for number in 1 2 3 4 5 6 7 8; do
  "$CHROME" \
    --headless=new \
    --disable-gpu \
    --hide-scrollbars \
    --force-device-scale-factor=1 \
    --window-size=2880,1800 \
    --screenshot="$OUTPUT_DIR/0${number}-lingstack.png" \
    "file://$HTML_PATH?slide=$number"
done

echo "$OUTPUT_DIR"
