#!/bin/zsh

set -u

: "${APP_STORE_CONNECT_KEY_ID:?Set APP_STORE_CONNECT_KEY_ID}"
: "${APP_STORE_CONNECT_ISSUER_ID:?Set APP_STORE_CONNECT_ISSUER_ID}"
: "${APP_STORE_CONNECT_KEY_PATH:?Set APP_STORE_CONNECT_KEY_PATH}"
: "${APP_STORE_VERSION_ID:?Set APP_STORE_VERSION_ID}"

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
screenshot_dir="${APP_STORE_SCREENSHOT_DIR:-$repository_root/marketing/store-preview/output/zh-CN}"
locale="${APP_STORE_SCREENSHOT_LOCALE:-zh-Hans}"
display_type="${APP_STORE_SCREENSHOT_DISPLAY_TYPE:-APP_DESKTOP}"
excluded_names=",${APP_STORE_SCREENSHOT_EXCLUDE:-03-lingstack.png},"
api_base="https://api.appstoreconnect.apple.com/v1"
jq_bin="$(command -v jq)"
curl_bin="$(command -v curl)"

jwt_token="$(xcrun altool --generate-jwt \
  --apiKey "$APP_STORE_CONNECT_KEY_ID" \
  --apiIssuer "$APP_STORE_CONNECT_ISSUER_ID" \
  --p8-file-path "$APP_STORE_CONNECT_KEY_PATH" 2>&1 | awk '/^eyJ/{print; exit}')"

if [[ -z "$jwt_token" ]]; then
  print -u2 "Unable to generate an App Store Connect JWT."
  exit 1
fi

api_request() {
  local method="$1"
  local endpoint="$2"
  local payload="${3:-}"
  local args=(-sS -X "$method" -H "Authorization: Bearer $jwt_token")
  if [[ -n "$payload" ]]; then
    args+=(-H "Content-Type: application/json" -d "$payload")
  fi
  "$curl_bin" "${args[@]}" "$api_base$endpoint"
}

localizations="$(api_request GET "/appStoreVersions/$APP_STORE_VERSION_ID/appStoreVersionLocalizations?limit=50")"
localization_id="$(print -r -- "$localizations" | "$jq_bin" -r --arg locale "$locale" '.data[] | select(.attributes.locale==$locale) | .id' | head -1)"
if [[ -z "$localization_id" ]]; then
  print -u2 "No App Store version localization found for $locale."
  exit 1
fi

sets="$(api_request GET "/appStoreVersionLocalizations/$localization_id/appScreenshotSets?limit=50")"
set_id="$(print -r -- "$sets" | "$jq_bin" -r --arg display "$display_type" '.data[] | select(.attributes.screenshotDisplayType==$display) | .id' | head -1)"
if [[ -z "$set_id" ]]; then
  payload="$("$jq_bin" -n \
    --arg display "$display_type" \
    --arg localization_id "$localization_id" \
    '{data:{type:"appScreenshotSets",attributes:{screenshotDisplayType:$display},relationships:{appStoreVersionLocalization:{data:{type:"appStoreVersionLocalizations",id:$localization_id}}}}}')"
  response="$(api_request POST "/appScreenshotSets" "$payload")"
  if print -r -- "$response" | "$jq_bin" -e '.errors' >/dev/null 2>&1; then
    print -u2 "Unable to create screenshot set: $(print -r -- "$response" | "$jq_bin" -c '.errors')"
    exit 1
  fi
  set_id="$(print -r -- "$response" | "$jq_bin" -r '.data.id')"
fi

screenshot_files=()
for candidate in "$screenshot_dir"/*.png(N); do
  file_name="$(basename "$candidate")"
  if [[ "$excluded_names" == *",$file_name,"* ]]; then
    print "Skipping excluded App Store screenshot: $file_name"
    continue
  fi
  screenshot_files+=("$candidate")
done
if (( ${#screenshot_files[@]} == 0 )); then
  print -u2 "No eligible PNG screenshots found in $screenshot_dir."
  exit 1
fi

existing="$(api_request GET "/appScreenshotSets/$set_id/appScreenshots?limit=200")"
existing_count="$(print -r -- "$existing" | "$jq_bin" -r '.data | length')"
if (( existing_count > 0 )); then
  if [[ "${APP_STORE_REPLACE_SCREENSHOTS:-0}" != "1" ]]; then
    print -u2 "Screenshot set $set_id already contains $existing_count image(s); set APP_STORE_REPLACE_SCREENSHOTS=1 to replace them."
    exit 2
  fi
  while IFS= read -r screenshot_id; do
    [[ -n "$screenshot_id" ]] || continue
    response="$(api_request DELETE "/appScreenshots/$screenshot_id")"
    if [[ -n "$response" ]] && print -r -- "$response" | "$jq_bin" -e '.errors' >/dev/null 2>&1; then
      print -u2 "Unable to delete screenshot $screenshot_id: $(print -r -- "$response" | "$jq_bin" -c '.errors')"
      exit 1
    fi
  done < <(print -r -- "$existing" | "$jq_bin" -r '.data[].id')
  remaining="$(api_request GET "/appScreenshotSets/$set_id/appScreenshots?limit=200" | "$jq_bin" -r '.data | length')"
  if (( remaining != 0 )); then
    print -u2 "Screenshot replacement stopped because $remaining existing image(s) remain."
    exit 1
  fi
fi

uploaded_ids=()
for screenshot_file in "${screenshot_files[@]}"; do
  file_name="$(basename "$screenshot_file")"
  file_size="$(stat -f %z "$screenshot_file")"
  payload="$("$jq_bin" -n \
    --arg file_name "$file_name" \
    --argjson file_size "$file_size" \
    --arg set_id "$set_id" \
    '{data:{type:"appScreenshots",attributes:{fileSize:$file_size,fileName:$file_name},relationships:{appScreenshotSet:{data:{type:"appScreenshotSets",id:$set_id}}}}}')"
  reservation="$(api_request POST "/appScreenshots" "$payload")"
  if print -r -- "$reservation" | "$jq_bin" -e '.errors' >/dev/null 2>&1; then
    print -u2 "$file_name reservation failed: $(print -r -- "$reservation" | "$jq_bin" -c '.errors')"
    exit 1
  fi

  screenshot_id="$(print -r -- "$reservation" | "$jq_bin" -r '.data.id')"
  operation_count="$(print -r -- "$reservation" | "$jq_bin" -r '.data.attributes.uploadOperations | length')"
  operation_index=0
  while (( operation_index < operation_count )); do
    operation="$(print -r -- "$reservation" | "$jq_bin" -c --argjson index "$operation_index" '.data.attributes.uploadOperations[$index]')"
    method="$(print -r -- "$operation" | "$jq_bin" -r '.method')"
    upload_url="$(print -r -- "$operation" | "$jq_bin" -r '.url')"
    offset="$(print -r -- "$operation" | "$jq_bin" -r '.offset')"
    length="$(print -r -- "$operation" | "$jq_bin" -r '.length')"
    upload_args=(-sS -o /dev/null -w '%{http_code}' -X "$method")
    while IFS=$'\t' read -r header_name header_value; do
      [[ -n "$header_name" ]] && upload_args+=(-H "$header_name: $header_value")
    done < <(print -r -- "$operation" | "$jq_bin" -r '.requestHeaders[]? | [.name,.value] | @tsv')

    status_code="$(/usr/bin/perl -e 'use strict; use warnings; my ($file,$offset,$length)=@ARGV; open my $fh,"<:raw",$file or die $!; seek $fh,$offset,0 or die $!; my $remaining=$length; while ($remaining>0) { my $chunk=""; my $read=read($fh,$chunk,$remaining>1048576?1048576:$remaining); die $! unless defined $read; last if $read==0; print $chunk; $remaining-=$read; } die "short read" if $remaining;' "$screenshot_file" "$offset" "$length" | "$curl_bin" "${upload_args[@]}" --data-binary @- "$upload_url")"
    if [[ "$status_code" != 2* ]]; then
      print -u2 "$file_name part $operation_index upload failed with HTTP $status_code."
      exit 1
    fi
    operation_index=$((operation_index + 1))
  done

  checksum="$(md5 -q "$screenshot_file")"
  payload="$("$jq_bin" -n --arg id "$screenshot_id" --arg checksum "$checksum" '{data:{type:"appScreenshots",id:$id,attributes:{uploaded:true,sourceFileChecksum:$checksum}}}')"
  committed="$(api_request PATCH "/appScreenshots/$screenshot_id" "$payload")"
  if print -r -- "$committed" | "$jq_bin" -e '.errors' >/dev/null 2>&1; then
    print -u2 "$file_name commit failed: $(print -r -- "$committed" | "$jq_bin" -c '.errors')"
    exit 1
  fi
  uploaded_ids+=("$screenshot_id")
  print "$file_name uploaded"
done

for screenshot_id in "${uploaded_ids[@]}"; do
  attempts=0
  while (( attempts < 24 )); do
    response="$(api_request GET "/appScreenshots/$screenshot_id")"
    state="$(print -r -- "$response" | "$jq_bin" -r '.data.attributes.assetDeliveryState.state // "UNKNOWN"')"
    if [[ "$state" == "COMPLETE" ]]; then
      break
    fi
    if [[ "$state" == "FAILED" ]]; then
      print -u2 "Screenshot $screenshot_id processing failed: $(print -r -- "$response" | "$jq_bin" -c '.data.attributes.assetDeliveryState.errors')"
      exit 1
    fi
    sleep 5
    attempts=$((attempts + 1))
  done
  if [[ "$state" != "COMPLETE" ]]; then
    print -u2 "Screenshot $screenshot_id is still in state $state."
    exit 1
  fi
done

print "Uploaded and processed ${#uploaded_ids[@]} App Store screenshots for $locale; exclusions: ${APP_STORE_SCREENSHOT_EXCLUDE:-03-lingstack.png}."
