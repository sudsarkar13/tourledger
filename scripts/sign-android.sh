#!/usr/bin/env bash
# Signs the Android release APK with your own key, creating the key the first
# time. keytool and apksigner ask for the password themselves; this script never
# sees or stores it.
#
# Keep a copy of the key file and its password somewhere safe. Without both, no
# later version can be installed over this one.
set -euo pipefail
cd "$(dirname "$0")/.."

key="${TOURLEDGER_KEYSTORE:-$HOME/.tourledger/release.keystore}"
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
tools="$sdk/build-tools/$(ls "$sdk/build-tools" | sort -V | tail -1)"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
unsigned=src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
out="dist/TourLedger_${version}_android.apk"

[ -f "$unsigned" ] || { echo "Build it first: scripts/build.sh android-release --target aarch64 armv7" >&2; exit 1; }

if [ ! -f "$key" ]; then
  echo "No key yet. Creating $key"
  echo "Choose a password (6 characters or more) and keep it somewhere safe."
  mkdir -p "$(dirname "$key")"
  chmod 700 "$(dirname "$key")"
  keytool -genkeypair -keystore "$key" -alias tourledger \
    -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=TourLedger"
  chmod 600 "$key"
fi

mkdir -p dist
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
aligned="$work/aligned.apk"
"$tools/zipalign" -f -P 16 4 "$unsigned" "$aligned"
echo "Signing: enter the key's password."
"$tools/apksigner" sign --ks "$key" --ks-key-alias tourledger --out "$out" "$aligned"
rm -f "$out.idsig"
"$tools/apksigner" verify --print-certs "$out" | grep -E 'SHA-256'
echo "Signed: $PWD/$out"
echo "Back up $key and its password."
