#!/bin/bash
set -euo pipefail
app_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$app_dir/../.." && pwd)"
mode="${1:-simulator}"
simulator_destination="${IOS_SIMULATOR_DESTINATION:-}"
archive_path="${IOS_ARCHIVE_PATH:-build/MeridianLab.xcarchive}"
export_path="${IOS_EXPORT_PATH:-build/export}"
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'Building iOS requires macOS and Xcode with the target iOS SDK.' >&2
  exit 1
fi
command -v xcodegen >/dev/null || { echo 'Install XcodeGen on this Mac first.' >&2; exit 1; }
xcodebuild -version
if [[ "$mode" == preflight ]]; then
  command -v rustup >/dev/null || { echo 'Install Rust/rustup on this Mac first.' >&2; exit 1; }
  command -v cargo >/dev/null || { echo 'Install Cargo on this Mac first.' >&2; exit 1; }
  command -v python3 >/dev/null || { echo 'Install Python 3 on this Mac first.' >&2; exit 1; }
  xcodebuild -showsdks | grep -q iphonesimulator || { echo 'An iOS Simulator SDK is required.' >&2; exit 1; }
  echo 'Apple build prerequisites are available.'
  exit 0
fi
cd "$repo_dir"
case "$mode" in
  simulator|simulator-test)
    if [[ "$(uname -m)" == arm64 ]]; then rust_target=aarch64-apple-ios-sim; else rust_target=x86_64-apple-ios; fi
    rustup target add "$rust_target"
    cargo build --locked --release --package meridian-mobile-core --target "$rust_target"
    cd "$app_dir"
    xcodegen generate --spec project.yml
    xcodebuild -project MeridianLab.xcodeproj -scheme MeridianLab -configuration Debug -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath build/DerivedData ONLY_ACTIVE_ARCH=YES MERIDIAN_RUST_TARGET="$rust_target" CODE_SIGNING_ALLOWED=NO build
    if [[ "$mode" == simulator-test ]]; then
      if [[ -z "$simulator_destination" ]]; then
        simulator_id="$(xcrun simctl list devices available | sed -n 's/.*(\([A-F0-9-]\{36\}\)).*/\1/p' | head -n 1)"
        [[ -n "$simulator_id" ]] || { echo 'No available iOS Simulator device found; set IOS_SIMULATOR_DESTINATION.' >&2; exit 1; }
        simulator_destination="id=$simulator_id"
      fi
      xcodebuild -project MeridianLab.xcodeproj -scheme MeridianLab -configuration Debug -sdk iphonesimulator -destination "$simulator_destination" -derivedDataPath build/DerivedData MERIDIAN_RUST_TARGET="$rust_target" CODE_SIGNING_ALLOWED=NO test
    fi
    ;;
  device-archive)
    rustup target add aarch64-apple-ios
    cargo build --locked --release --package meridian-mobile-core --target aarch64-apple-ios
    cd "$app_dir"
    xcodegen generate --spec project.yml
    xcodebuild -project MeridianLab.xcodeproj -scheme MeridianLab -configuration Release -destination 'generic/platform=iOS' -archivePath "$archive_path" MERIDIAN_RUST_TARGET=aarch64-apple-ios CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO archive
    ;;
  archive)
    : "${APPLE_TEAM_ID:?Set APPLE_TEAM_ID to your legitimate development team identifier}"
    rustup target add aarch64-apple-ios
    cargo build --locked --release --package meridian-mobile-core --target aarch64-apple-ios
    cd "$app_dir"
    xcodegen generate --spec project.yml
    xcodebuild -project MeridianLab.xcodeproj -scheme MeridianLab -configuration Release -destination 'generic/platform=iOS' -archivePath "$archive_path" DEVELOPMENT_TEAM="$APPLE_TEAM_ID" MERIDIAN_RUST_TARGET=aarch64-apple-ios archive
    xcodebuild -exportArchive -archivePath "$archive_path" -exportOptionsPlist "${IOS_EXPORT_OPTIONS:-export-development.plist}" -exportPath "$export_path" DEVELOPMENT_TEAM="$APPLE_TEAM_ID"
    mapfile -t ipa_files < <(find "$export_path" -maxdepth 1 -type f -name '*.ipa' -print)
    [[ "${#ipa_files[@]}" -eq 1 ]] || { echo "Expected exactly one IPA in $export_path." >&2; exit 1; }
    ipa="${ipa_files[0]}"
    info_plist="$(mktemp -t meridian-info).plist"
    trap 'rm -f "$info_plist"' EXIT
    unzip -p "$ipa" 'Payload/*.app/Info.plist' > "$info_plist"
    bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$info_plist")"
    mobile_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$info_plist")"
    [[ "$bundle_id" == dev.meridianlab.ios ]] || { echo "Unexpected bundle identifier: $bundle_id" >&2; exit 1; }
    [[ -n "$mobile_version" ]] || { echo 'IPA is missing CFBundleShortVersionString.' >&2; exit 1; }
    ipa_size="$(stat -f '%z' "$ipa")"
    ipa_sha256="$(shasum -a 256 "$ipa" | awk '{print $1}')"
    python3 - "$ipa" "$ipa_size" "$ipa_sha256" "$bundle_id" "$mobile_version" "${IOS_MANIFEST_PATH:-$export_path/MeridianLab.manifest.json}" <<'PY'
import json, pathlib, sys
ipa, size, digest, bundle_id, version, output = sys.argv[1:]
manifest = {"mobileVersion": version, "bundleIdentifier": bundle_id,
            "pairingSchemaVersion": 2, "minimumDesktopVersion": "0.1.0",
            "size": int(size), "sha256": digest.lower()}
pathlib.Path(output).write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
print(f'IPA={ipa}')
print(f'MANIFEST={output}')
PY
    ;;
  bridge)
    cd "$app_dir"
    xcodegen generate --spec project.yml
    xcodebuild -project MeridianLab.xcodeproj -scheme MeridianBridge -configuration Debug -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath build/DerivedData CODE_SIGNING_ALLOWED=NO build
    ;;
  *) echo 'Usage: bash apps/ios/build.sh preflight|simulator|simulator-test|device-archive|archive|bridge' >&2; exit 2 ;;
esac
