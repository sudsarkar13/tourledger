#!/usr/bin/env bash
# Builds TourLedger for one platform from this machine.
#
#   scripts/build.sh            the desktop system this is run on
#   scripts/build.sh android    APK for testing (needs the Android SDK and NDK)
#   scripts/build.sh android --target aarch64    only for 64-bit phones, faster
#   scripts/build.sh ios        simulator build (needs macOS with full Xcode)
#
# Windows, Linux and FreeBSD builds run on those systems or in CI
# (.github/workflows/build.yml). Nothing is uploaded or published.
set -euo pipefail
cd "$(dirname "$0")/.."

tauri=ui/node_modules/.bin/tauri
pnpm --dir ui install --frozen-lockfile

case "${1:-desktop}" in
  desktop)
    case "$(uname -s)" in
      Darwin)
        "$tauri" build --bundles app,dmg
        app=target/release/bundle/macos/TourLedger.app
        # A bundle that fails this installs but may not open or be listed.
        codesign --verify --deep --strict "$app"
        # Building registers copies of the app with macOS under the same ID as
        # the installed one. Remove them so only /Applications is listed.
        lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
        "$lsregister" -u "$PWD/$app" 2>/dev/null || true
        { "$lsregister" -dump | sed -nE 's|^path: +(/Volumes/dmg\.[^/]+/TourLedger\.app).*|\1|p' | sort -u |
          while read -r stale; do "$lsregister" -u "$stale" || true; done; } 2>/dev/null
        echo "Built: $PWD/target/release/bundle/dmg"
        ;;
      Linux) "$tauri" build --bundles deb,appimage ;;
      FreeBSD) "$tauri" build --no-bundle ;;
      MINGW* | MSYS* | CYGWIN*) "$tauri" build --bundles nsis ;;
      *) echo "No build recipe for $(uname -s)." >&2; exit 1 ;;
    esac
    ;;
  android)
    : "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK folder}"
    # The newest installed NDK, unless one is already chosen.
    NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/$(ls "$ANDROID_HOME/ndk" | sort -V | tail -1)}"
    [ -d "$NDK_HOME/toolchains" ] || { echo "No Android NDK found. Install one in Android Studio's SDK Manager." >&2; exit 1; }
    export NDK_HOME
    [ -d src-tauri/gen/android ] || "$tauri" android init --ci
    "$tauri" icon src-tauri/icons/icon.svg -o src-tauri/icons
    # No cloud backup: it would copy this device's sync key and the ledger.
    manifest=src-tauri/gen/android/app/src/main/AndroidManifest.xml
    if ! grep -q 'android:allowBackup="false"' "$manifest"; then
      sed -i.bak 's/<application/<application android:allowBackup="false"/' "$manifest" && rm "$manifest.bak"
    fi
    # The generated Gradle task runs "node tauri" from src-tauri, which only
    # works for a project with the CLI at its root. Point it at the one in ui/.
    task=$(find src-tauri/gen/android/buildSrc -name BuildTask.kt)
    cli=../ui/node_modules/@tauri-apps/cli/tauri.js
    if ! grep -q "$cli" "$task"; then
      sed -i.bak "s|listOf(\"tauri\", \"android\"|listOf(\"$cli\", \"android\"|" "$task" && rm "$task.bak"
      grep -q "$cli" "$task" || { echo "Could not point Gradle at the Tauri CLI." >&2; exit 1; }
    fi
    # Android hides other apps unless asked about: without this "Take photo"
    # finds no camera and silently opens the photo picker instead.
    if ! grep -q 'android.media.action.IMAGE_CAPTURE' "$manifest"; then
      sed -i.bak 's|<application |<queries><intent><action android:name="android.media.action.IMAGE_CAPTURE" /></intent></queries>\
    <application |' "$manifest" && rm "$manifest.bak"
      grep -q 'android.media.action.IMAGE_CAPTURE' "$manifest" || { echo "Could not add the camera query." >&2; exit 1; }
    fi
    # Sync on the same Wi-Fi: Android 17 asks before an app may use the local
    # network, and multicast (how devices find each other) needs its own entry.
    for permission in ACCESS_LOCAL_NETWORK CHANGE_WIFI_MULTICAST_STATE; do
      if ! grep -q "android.permission.$permission" "$manifest"; then
        sed -i.bak "s|<uses-permission android:name=\"android.permission.INTERNET\" />|&\\
    <uses-permission android:name=\"android.permission.$permission\" />|" "$manifest" && rm "$manifest.bak"
        grep -q "android.permission.$permission" "$manifest" || { echo "Could not add $permission." >&2; exit 1; }
      fi
    done
    # Keeps the page clear of the system bars and the keyboard, and asks for
    # local network access when the page needs it.
    cp src-tauri/android/MainActivity.kt src-tauri/gen/android/app/src/main/java/app/tourledger/MainActivity.kt
    # Extra arguments go to Tauri, for example: --target aarch64
    "$tauri" android build --debug --apk "${@:2}"
    ;;
  ios)
    [ -d src-tauri/gen/apple ] || "$tauri" ios init --ci
    "$tauri" ios build --debug --target aarch64-sim
    ;;
  *)
    echo "Usage: scripts/build.sh [desktop|android|ios]" >&2
    exit 1
    ;;
esac
