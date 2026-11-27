# Platforms

One codebase. What has actually been done for each system, as of 27 November 2026:

| System | How it is built | Built so far? | Run and tested? |
|---|---|---|---|
| macOS (Apple silicon) | `scripts/build.sh` | Yes, ad-hoc signed | Installed and opened. Navigation and Save PDF checked inside the app; other screens in a browser against the same Rust code |
| macOS (Intel, universal) | CI | Yes | No |
| Windows x64, arm64 | CI, or `scripts/build.sh` on Windows | Yes, in CI | No; the core tests pass on x64 |
| Linux x64, arm64 (deb, rpm, AppImage) | CI, or `scripts/build.sh` on Linux | Yes, in CI | No; the core tests pass on x64 |
| Android (ARM, 64 and 32-bit) | `scripts/build.sh android-release`, then `scripts/sign-android.sh` | Yes, signed release APK | A release build on an Android 16 emulator: launch, preview, backup and restore, QR scanner opening. Not on a real phone; sync and reading a code not tried |
| iOS, iPadOS | `scripts/build.sh ios` (simulator), or CI | Simulator build only, in CI | No |
| FreeBSD | CI (experimental) | Compiles in CI | No |

CI runs for version tags and when started by hand.

## Releasing

A tag `v…` builds the desktop installers into a draft pre-release, with notes from
`docs/releases/<tag>.md`. The Android APK is built and signed on the owner's machine
with their own key, then added to the draft.

| System | Still missing |
|---|---|
| iOS | An Apple Developer account for real devices and the App Store |
| macOS | An Apple Developer ID to sign and notarise, so users see a warning |
| Windows | A code-signing certificate, so users see a warning |

iOS also needs Apple's multicast networking entitlement before devices can find
each other on Wi-Fi; until then it would sync over the internet only.

## Not covered

- **FreeBSD:** the window toolkit support is community-maintained; treat as
  experimental until it has been run on a real system.
- **OpenBSD, NetBSD and others:** no recipe. **ChromeOS** runs the Android app
  (untested). There is no web version.
