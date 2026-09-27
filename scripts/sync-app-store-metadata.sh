#!/bin/zsh

set -eu
set -o pipefail

: "${APP_STORE_CONNECT_KEY_ID:?Set APP_STORE_CONNECT_KEY_ID}"
: "${APP_STORE_CONNECT_ISSUER_ID:?Set APP_STORE_CONNECT_ISSUER_ID}"
: "${APP_STORE_CONNECT_KEY_PATH:?Set APP_STORE_CONNECT_KEY_PATH}"
: "${APP_STORE_INFO_ID:?Set APP_STORE_INFO_ID}"
: "${APP_STORE_VERSION_ID:?Set APP_STORE_VERSION_ID}"

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
metadata_root="${APP_STORE_METADATA_ROOT:-$repository_root/app-store/metadata}"
api_base="https://api.appstoreconnect.apple.com/v1"
jq_bin="$(command -v jq)"
curl_bin="$(command -v curl)"

expected_version="${APP_STORE_EXPECTED_VERSION:-$("$jq_bin" -er '.version | select(type == "string" and length > 0)' "$repository_root/package.json")}"
expected_app_id="${APP_STORE_EXPECTED_APP_ID:-6798135179}"
if [[ ! "$expected_version" =~ '^[0-9]+(\.[0-9]+){1,2}$' || ! "$expected_app_id" =~ '^[0-9]+$' ]]; then
  print -u2 "Expected version or app ID is invalid."
  exit 1
fi
review_notes_path="${APP_STORE_REVIEW_NOTES_PATH:-$repository_root/app-store/review-notes-${expected_version}.md}"
whats_new_path="$metadata_root/${expected_version}-whats-new.json"
[[ -s "$review_notes_path" && -s "$whats_new_path" ]] || {
  print -u2 "Missing version-specific review notes or What's New metadata."
  exit 1
}
"$jq_bin" -e 'type == "object"' "$whats_new_path" >/dev/null
# Validate every selected locale before the first remote mutation, avoiding partial updates.
typeset -a selected_locale_dirs
selected_locale_dirs=()
for locale_dir in "$metadata_root"/*; do
  [[ -d "$locale_dir" ]] || continue
  locale="$(basename "$locale_dir")"
  if [[ -n "${APP_STORE_LOCALES:-}" && ",${APP_STORE_LOCALES}," != *",${locale},"* ]]; then
    continue
  fi
  "$jq_bin" -e --arg locale "$locale" '.[$locale] | type == "string" and test("\\S")' "$whats_new_path" >/dev/null || {
    print -u2 "Missing or empty What's New for $locale."
    exit 1
  }
  selected_locale_dirs+=("$locale_dir")
done
(( ${#selected_locale_dirs} > 0 )) || { print -u2 "No selected metadata locales."; exit 1; }

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
  local args=(-fsS -X "$method" -H "Authorization: Bearer $jwt_token")
  if [[ -n "$payload" ]]; then
    args+=(-H "Content-Type: application/json" -d "$payload")
  fi
  "$curl_bin" "${args[@]}" "$api_base$endpoint"
}

# Pin writes to the explicitly expected release and application, never a stale version ID.
version_identity="$(api_request GET "/appStoreVersions/$APP_STORE_VERSION_ID?include=app")"
print -r -- "$version_identity" | "$jq_bin" -e \
  --arg id "$APP_STORE_VERSION_ID" --arg version "$expected_version" --arg app "$expected_app_id" \
  '.data.type == "appStoreVersions" and .data.id == $id and .data.attributes.versionString == $version and .data.relationships.app.data.type == "apps" and .data.relationships.app.data.id == $app' >/dev/null || {
  print -u2 "App Store version identity mismatch; no metadata was written."
  exit 1
}
# App information is a separate write target and must belong to the same app.
info_identity="$(api_request GET "/appInfos/$APP_STORE_INFO_ID?include=app")"
print -r -- "$info_identity" | "$jq_bin" -e --arg id "$APP_STORE_INFO_ID" --arg app "$expected_app_id" \
  '.data.type == "appInfos" and .data.id == $id and .data.relationships.app.data.type == "apps" and .data.relationships.app.data.id == $app' >/dev/null || {
  print -u2 "App information identity mismatch; no metadata was written."
  exit 1
}

strip_trailing_newlines='sub("\\n+$";"")'
app_info_response="$(api_request GET "/appInfos/$APP_STORE_INFO_ID/appInfoLocalizations?limit=50")"
version_response="$(api_request GET "/appStoreVersions/$APP_STORE_VERSION_ID/appStoreVersionLocalizations?limit=50")"
failures=0

for locale_dir in "${selected_locale_dirs[@]}"; do
  locale="$(basename "$locale_dir")"
  whats_new="$("$jq_bin" -er --arg locale "$locale" '.[$locale]' "$whats_new_path")"
  info_id="$(print -r -- "$app_info_response" | "$jq_bin" -r --arg locale "$locale" '.data[] | select(.attributes.locale==$locale) | .id' | head -1)"
  version_localization_id="$(print -r -- "$version_response" | "$jq_bin" -r --arg locale "$locale" '.data[] | select(.attributes.locale==$locale) | .id' | head -1)"

  if [[ -n "$info_id" ]]; then
    payload="$("$jq_bin" -n \
      --arg id "$info_id" \
      --rawfile name "$locale_dir/name.txt" \
      --rawfile subtitle "$locale_dir/subtitle.txt" \
      --rawfile privacy "$locale_dir/privacy_url.txt" \
      --arg strip "$strip_trailing_newlines" \
      '{data:{type:"appInfoLocalizations",id:$id,attributes:{name:($name|sub("\\n+$";"")),subtitle:($subtitle|sub("\\n+$";"")),privacyPolicyUrl:($privacy|sub("\\n+$";""))}}}')"
    response="$(api_request PATCH "/appInfoLocalizations/$info_id" "$payload")"
  else
    payload="$("$jq_bin" -n \
      --arg locale "$locale" \
      --arg app_info_id "$APP_STORE_INFO_ID" \
      --rawfile name "$locale_dir/name.txt" \
      --rawfile subtitle "$locale_dir/subtitle.txt" \
      --rawfile privacy "$locale_dir/privacy_url.txt" \
      '{data:{type:"appInfoLocalizations",attributes:{locale:$locale,name:($name|sub("\\n+$";"")),subtitle:($subtitle|sub("\\n+$";"")),privacyPolicyUrl:($privacy|sub("\\n+$";""))},relationships:{appInfo:{data:{type:"appInfos",id:$app_info_id}}}}}')"
    response="$(api_request POST "/appInfoLocalizations" "$payload")"
    info_id="$(print -r -- "$response" | "$jq_bin" -r '.data.id // empty')"
  fi

  if print -r -- "$response" | "$jq_bin" -e '.errors' >/dev/null 2>&1; then
    print -u2 "$locale app-info error: $(print -r -- "$response" | "$jq_bin" -c '.errors')"
    failures=$((failures + 1))
    continue
  fi

  if [[ -n "$version_localization_id" ]]; then
    payload="$("$jq_bin" -n \
      --arg id "$version_localization_id" \
      --arg whats_new "$whats_new" \
      --rawfile description "$locale_dir/description.txt" \
      --rawfile keywords "$locale_dir/keywords.txt" \
      --rawfile marketing "$locale_dir/marketing_url.txt" \
      --rawfile promotional "$locale_dir/promotional_text.txt" \
      --rawfile support "$locale_dir/support_url.txt" \
      '{data:{type:"appStoreVersionLocalizations",id:$id,attributes:{whatsNew:$whats_new,description:($description|sub("\\n+$";"")),keywords:($keywords|sub("\\n+$";"")),marketingUrl:(($marketing|sub("\\n+$";"")) | if length == 0 then null else . end),promotionalText:($promotional|sub("\\n+$";"")),supportUrl:($support|sub("\\n+$";""))}}}')"
    response="$(api_request PATCH "/appStoreVersionLocalizations/$version_localization_id" "$payload")"
  else
    payload="$("$jq_bin" -n \
      --arg locale "$locale" \
      --arg version_id "$APP_STORE_VERSION_ID" \
      --arg whats_new "$whats_new" \
      --rawfile description "$locale_dir/description.txt" \
      --rawfile keywords "$locale_dir/keywords.txt" \
      --rawfile marketing "$locale_dir/marketing_url.txt" \
      --rawfile promotional "$locale_dir/promotional_text.txt" \
      --rawfile support "$locale_dir/support_url.txt" \
      '{data:{type:"appStoreVersionLocalizations",attributes:{locale:$locale,whatsNew:$whats_new,description:($description|sub("\\n+$";"")),keywords:($keywords|sub("\\n+$";"")),marketingUrl:(($marketing|sub("\\n+$";"")) | if length == 0 then null else . end),promotionalText:($promotional|sub("\\n+$";"")),supportUrl:($support|sub("\\n+$";""))},relationships:{appStoreVersion:{data:{type:"appStoreVersions",id:$version_id}}}}}')"
    response="$(api_request POST "/appStoreVersionLocalizations" "$payload")"
    version_localization_id="$(print -r -- "$response" | "$jq_bin" -r '.data.id // empty')"
  fi

  if print -r -- "$response" | "$jq_bin" -e '.errors' >/dev/null 2>&1; then
    print -u2 "$locale version error: $(print -r -- "$response" | "$jq_bin" -c '.errors')"
    failures=$((failures + 1))
    continue
  fi

  print "$locale synced"
done

if [[ -f "$review_notes_path" ]]; then
  review_detail_response="$(api_request GET "/appStoreVersions/$APP_STORE_VERSION_ID/appStoreReviewDetail")"
  review_detail_id="$(print -r -- "$review_detail_response" | "$jq_bin" -r '.data.id // empty')"
  if [[ -z "$review_detail_id" ]]; then
    print -u2 "Unable to locate the App Store review detail for $APP_STORE_VERSION_ID."
    failures=$((failures + 1))
  else
    payload="$("$jq_bin" -n \
      --arg id "$review_detail_id" \
      --rawfile notes "$review_notes_path" \
      '{data:{type:"appStoreReviewDetails",id:$id,attributes:{notes:($notes|sub("\\n+$";""))}}}')"
    response="$(api_request PATCH "/appStoreReviewDetails/$review_detail_id" "$payload")"
    if print -r -- "$response" | "$jq_bin" -e '.errors' >/dev/null 2>&1; then
      print -u2 "review-notes error: $(print -r -- "$response" | "$jq_bin" -c '.errors')"
      failures=$((failures + 1))
    else
      print "review notes synced"
    fi
  fi
fi

exit "$failures"
