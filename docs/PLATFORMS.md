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

A tag `v…` builds the desktop installers, signs them with the update key, writes
`latest.json` and collects everything into a draft release, with notes from
`docs/releases/<tag>.md`. The Android APK is built and signed on the owner's machine
with their own key, then added to the draft. Publishing it as the latest release (not
a pre-release) is what offers it to installed apps.

1. Set the version in `Cargo.toml`, `src-tauri/tauri.conf.json` and `ui/package.json`,
   and write the notes. Commit, tag `v…` and push the tag.
2. `scripts/build.sh android-release --target aarch64 armv7`, then
   `scripts/sign-android.sh`.
3. Attach the APK to the draft **before publishing**, or Android users are told there
   is nothing new.
4. `gh release edit v… --draft=false --latest`

Two keys, both outside the repository in `~/.tourledger`, both needing a backup:
`release.keystore` signs the APK, `updater.key` signs desktop updates (a copy is held
as a GitHub secret). Losing either means installed copies of that kind can never be
updated again. The updater key was set up with `scripts/setup-update-key.sh`.

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
