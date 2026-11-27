# Making a Tauri app install properly on macOS

Reusable for any Tauri project. Findings from 27 November 2026, macOS 27.2.

## 1. Seal the whole bundle

In `src-tauri/tauri.conf.json`:

```json
"bundle": {
  "macOS": { "signingIdentity": "-" }
}
```

Without this only the binary is signed (by the linker) and the bundle is invalid:
`code has no resources but signature indicates they must be present`. A downloaded
copy then reads as "damaged". Set `APPLE_SIGNING_IDENTITY` to override with a real
Developer ID.

Check every build, and fail on error:

```bash
codesign --verify --deep --strict path/to/App.app
```

## 2. Do not end the identifier in `.app`

It clashes with the bundle extension. For Android, no part may be a Java or Kotlin
keyword (`in`, `new`, `class`, …). Choose it before release: changing it later
moves the data folder and makes a different Android app.

## 3. Leave no second copy registered

Building registers the copy in `target/` (and temporary disk-image mounts) with
macOS under the same ID as the installed app. `scripts/build.sh` removes them:

```bash
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -u path/to/target/App.app
```

## 4. Open it once after installing

A freshly copied app was marked `launch-disabled` by macOS even with a valid
signature. The mark cleared the first time it was opened. Drag it to
Applications, then open it from the Applications folder in Finder.

## Still needed for other people's Macs

An ad-hoc signature does not satisfy Gatekeeper on a downloaded file: they see an
"unidentified developer" warning. Removing it needs an Apple Developer ID and
notarisation (a paid Apple Developer account).
