# Platforms

One codebase. What has actually been done for each system, as of 27 November 2026:

| System | How it is built | Built so far? | Run and tested? |
|---|---|---|---|
| macOS (Apple silicon) | `scripts/build.sh` | Yes, ad-hoc signed | Installed and opened. Navigation and Save PDF checked inside the app; other screens in a browser against the same Rust code |
| macOS (Intel, universal) | CI | No | No |
| Windows x64, arm64 | CI, or `scripts/build.sh` on Windows | No (core compiles for Windows) | No |
| Linux x64, arm64 (deb, rpm, AppImage) | CI, or `scripts/build.sh` on Linux | No (core compiles for Linux) | No |
| Android (64-bit) | `scripts/build.sh android`, or CI | Yes, debug test APK | On an Android 16 emulator: launch, saving, camera, preview, Save PDF, back button. Not on a real phone; sync and QR scanning not tried |
| iOS, iPadOS | `scripts/build.sh ios` (simulator), or CI | No (core compiles for iOS) | No |
| FreeBSD | CI (experimental) | No (core compiles for FreeBSD) | No |

"Core compiles" means `cargo check -p tourledger-core --target …` passed: storage,
money, claims, loans and the PDF. For Windows, Linux, iOS and FreeBSD it was run
before bill pictures were added. It says nothing about networking or the window,
which need each system's own tools.

## What is needed before release

| System | Needed |
|---|---|
| All CI builds | The project pushed to a GitHub account of your choice |
| Android release | A signing key you create and keep; the current APK is a large debug build |
| iOS | Full Xcode; an Apple Developer account for real devices and the App Store |
| macOS | An Apple Developer ID to sign and notarise, or users see a warning |
| Windows | A code-signing certificate, or users see a warning |

iOS also needs Apple's multicast networking entitlement before devices can find
each other on Wi-Fi; until then it would sync over the internet only.

## Not covered

- **FreeBSD:** the window toolkit support is community-maintained; treat as
  experimental until it has been run on a real system.
- **OpenBSD, NetBSD and others:** no recipe. **ChromeOS** runs the Android app
  (untested). There is no web version.
