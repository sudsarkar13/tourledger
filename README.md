# TourLedger

Track what a tour or event cost, what the company owes you after advances, your own
spending, and your loans. One person, several devices, no account.

Rust core with its own storage format · Tauri v2 · React.

## What it does

**Tours and expenses**

- Home shows one company (or your personal trips) at a time, never mixed
- Quick entry: type `cab home to station 180`, `lunch with Ravi 640`, `company gave 2000`;
  check what was understood, then save. Rules, no AI
- Trips for business, wellness, reward/bonus, or personal
- A large calendar with month and year choice; journey days before or after the event
  are marked and totalled separately in the report
- A note picks the category (`powder tea` is Food); your own earlier choice for the
  same note wins, and a saved category changes only when you say so
- Cash the company gave, bonuses kept, reimbursements, and what is still due either way
- Claim report with totals by category (Travelling expenses, Food and refreshments,
  Stay, Event, Miscellaneous), or every entry if the company asks; per person,
  saved as PDF or spreadsheet. Personal items never appear on a claim
- Photograph or add bills to an expense; they follow the claim, four to a page, each
  labelled with its transaction. Preview every page before saving
- Companies with your role and colleagues at each, so changing jobs keeps old claims

**Loans (kept apart; nothing about them on Home)**

- Dashboard: owed, due this month, overdue, next 45 days
- My loans: running and closed, grouped by kind; close and reopen
- EMI tracker: every month of every loan on one page; tap to mark paid or not
- Insights: interest paid and to come, debt-free date, EMIs as a share of income

**Your devices**

- Own storage format, on the device; sync between your own devices, off until you
  turn it on

## Use it on two devices

1. On the first device: **More → Sync → Show my code**.
2. On the second: **More → Sync → Enter a code** (or **Scan a code** on Android).
3. Both need the app open to exchange changes.

To stop: switch sync off, or unlink a device (the link ends on both). To keep one tour or loan private, turn
on **this device only** in its form.

## Develop

Needs Rust (stable), Node 24+ and pnpm.

```bash
pnpm --dir ui install
```

```bash
ui/node_modules/.bin/tauri dev
```

Test:

```bash
cargo test
```

```bash
pnpm --dir ui test
```

Sync over the public network (needs internet):

```bash
cargo test -p tourledger-sync -- --ignored
```

### UI in a plain browser

Run the development bridge and the UI server in two terminals, then open
http://127.0.0.1:1420. The bridge has no authentication and is for development only.

```bash
cargo run -p tourledger-devbridge
```

```bash
pnpm --dir ui dev
```

## Build

```bash
scripts/build.sh
```

builds for the desktop system you run it on. `scripts/build.sh android` and
`scripts/build.sh ios` need the Android SDK/NDK or full Xcode. CI
(`.github/workflows/build.yml`) covers macOS, Windows, Linux, Android, the iOS
simulator and an experimental FreeBSD job. It runs for version tags and when started
by hand.

Downloads are on the [releases page](https://github.com/sudsarkar13/tourledger/releases).

The Android test APK lands in
`src-tauri/gen/android/app/build/outputs/apk/universal/debug/`.

What has and has not been built for each system: [platforms](docs/PLATFORMS.md).
Installing on macOS: [recipe](docs/MACOS-INSTALL.md).

## Where your data is

| System | Folder |
|---|---|
| macOS | `~/Library/Application Support/app.tourledger` |
| Windows | `%APPDATA%\app.tourledger` |
| Linux | `~/.local/share/app.tourledger` |
| Android | the app's private storage |

`ledger.tlg` is your data, `bills/` holds the bill pictures, and `device.json` is this device's identity and keys. Neither
is encrypted by the app. Keep disk encryption on, and make backups from
**More → Backup and import**.

## Known limits

- Editing the same entry on two devices before they sync keeps the later edit only.
- A restore does not bring back items deleted after the backup was made, if the
  ledger still remembers the deletion.
- Bills are pictures only (no PDF invoices). No per-diem rules or notifications
  while the app is closed.
- The PDF cannot draw Indian scripts; such characters print as `?`.
- Amounts are shown in rupees; other currencies are converted at your entered rate.
- Nothing can erase data from a lost device. Unlinking works per pair: unlink the lost
  device on each of your other devices, or one of them will keep forwarding changes.

More: [architecture](docs/ARCHITECTURE.md) · [scenarios](docs/SCENARIOS.md) ·
[design rules](docs/DESIGN.md)

## License

[MIT](LICENSE)
