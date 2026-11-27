# TourLedger — architecture

A single-person tour, expense and loan tracker for macOS, Windows, Linux and Android.
No account, no login, no server that stores your data.

## Layers

```
ui/                 React + TypeScript. Renders; holds no business rules.
   │  one call: api(request) → response   (JSON)
src-tauri/          Thin native shell (Tauri v2). Wires files, dialogs, sync.
crates/core/        All rules: records, storage engine, money, settlement,
                    loans, insights, export. No network, no UI.
crates/sync/        Device-to-device sync over iroh (encrypted QUIC).
crates/devbridge/   Development only: serves the same api() on 127.0.0.1 so the
                    UI can be tested in a normal browser. Never shipped.
```

One request enum (`core::api::Request`) is the whole surface between UI and Rust.

## Storage engine (own format, no SQLite)

One append-only file, `ledger.tlg`:

```
header  "TLEDGER1"
frame   "TLF1" | payload length u32 | CRC32 of payload u32 | payload
```

A payload is one transaction: a list of record versions, applied all or nothing.
State is rebuilt in memory on open. Data size for one person is a few MB, so an
in-memory view is simpler and faster than on-disk indexes.

| Failure | Behaviour |
|---|---|
| Power loss mid-write | Incomplete last frame is cut off on next open |
| Damaged frame in the middle | App refuses to open and keeps the file untouched |
| Two app instances | Second one fails on an exclusive file lock |
| File grows from edits | Compaction rewrites to a temp file, then atomic rename |

Every commit is flushed to disk before the UI is told it succeeded.

**Not provided:** encryption at rest. The file relies on the operating system's
disk encryption (FileVault, BitLocker, Android app sandbox).

## Records and merging

Every item (tour, expense, loan, …) is a record: `kind, id, stamp, deleted, body`.
`stamp` is a hybrid logical clock `(milliseconds, counter, device)`.

Merge rule: for the same `(kind, id)`, the higher stamp wins. Applying records is
commutative, associative and idempotent, so devices converge whatever the order.

Known limit: if the **same record** is edited on two devices before they sync, the
later edit wins and the earlier one is replaced. Each advance, payment and
expense is its own record, so adding things on two devices never collides.

Deletes are kept as tombstones so a deleted item does not come back from a peer.

## Bill pictures

The UI shrinks each picture to a JPEG (short side up to 1200 px, long up to
4000 px). Rust checks it and stores it as `bills/<sha256>.jpg`; an `attachment`
record ties it to an expense. The ledger holds no image data. A file is removed
only when the person deletes the picture, its expense or its tour and nothing else
shows it. Sync sends records first, then any missing pictures, up to 20 each
time. Backups and sync files carry the pictures inside them.

The report is laid out once, as marks on pages; the PDF and the on-screen preview
both draw those marks.

## Sync

- Off by default. Nothing touches the network until it is turned on.
- Transport: [iroh](https://iroh.computer). Devices dial each other by public key,
  on the same Wi-Fi or across the internet, end-to-end encrypted. iroh's public
  relay servers help devices find each other and forward encrypted packets when a
  direct path is impossible. They cannot read the data; they can see that two
  device keys talk, and they are rate limited with no uptime promise.
- Pairing: one device shows a code (its key + a one-time secret, valid 10
  minutes). The other enters it. Only paired keys are ever accepted.
- Exchange: each side sends a list of `(kind, id, stamp)`, then only the records
  the other lacks.
- Cut off: turn sync off, or unlink a device. Each side keeps its own copy and
  they diverge from then on. Linking again merges them.
- Unlinking ends the link on both devices: the other is told at once, or, if it is
  away, when it next calls. Only a device deliberately unlinked is told to drop.
- Unlinking is per pair. With three or more devices, a device you unlink on one
  still gets changes through any other that stays linked to it. Unlink it on each.
- A backup restores device-only entries; a sync file and a peer never do.
- A claim belongs to a tour. If a tour is deleted on one device while another adds
  to it, those entries show as outside any tour, with a reminder.
- Segregation: a tour or loan can be marked **this device only**. It and
  everything under it is never sent. Copies a peer already received stay there.
- Fallback without network: export a sync file and import it on the other device.
- Both devices must be running the app for live sync. There is no cloud mailbox.
- Coming back to the app or onto a network sends `sync.wake`, which tells iroh the
  network changed (phones do not) and contacts every linked device at once.
- The status says what each link is doing (connecting, comparing, pictures) and
  what the last exchange moved. These are kept in memory, not synced.
- Same Wi-Fi: Android 17 needs the local network permission (asked when sync is
  first used) and a multicast lock, held while the app is in front; macOS shows
  its own local network prompt. Without them sync still works over the internet.

## Updating

The program is replaced; the data folder is never part of it. The page only starts
steps and shows progress (`ui/src/updates.ts`); where updates come from is fixed in
native code and configuration, so the page cannot redirect it.

- **macOS, Windows, Linux** (`src-tauri/src/updates.rs`): Tauri's updater reads
  `latest.json` from the latest GitHub release and installs only a file signed with
  the update key, whose signature also names the version, so an old release cannot be
  offered as a new one.
- **Android** (`src-tauri/android/Updater.kt`): reads the latest release from GitHub,
  downloads the APK, checks its size and SHA-256, and hands it to Android's package
  installer for this package only. Android itself refuses an APK signed with another
  key or with a lower version.
- **When:** shortly after opening and daily while open, unless switched off. It
  downloads by itself except on a metered connection, and installs only when asked.
- Anyone who controls the GitHub account can publish an update. Keep two-factor
  sign-in on it.

A change to the stored or synced format must stay readable by the previous version:
linked devices update at different times.

## Money

Integer minor units (paise) everywhere. No floating point in sums. A foreign
currency expense stores its own exchange rate, fixed at entry.

## Settlement for a company tour

```
claimable = my reimbursable expenses (rejected ones excluded)
net       = claimable − advance received + advance returned − reimbursed
bonus or allowance from the company is kept: it never changes net
net > 0   company owes me
net < 0   I owe the company
```

Company-paid items (for example, rail tickets) count in the tour's total cost but
never in the claim. Colleague-paid items are in neither claim nor my spending.

## Typed lines

`core::entry` reads one line per item with fixed rules. It never saves: the person
sees each reading first. It reads a line as company money only when every word in
it is about money, and refuses a line whose payer is a name it does not know.

## Two areas

Tour figures (`insights`) and loan figures (`portfolio`) are computed separately;
no loan value enters a tour total, reminder or estimate.

## Reminders

`core::reminders` lists which unpaid instalments to mention and on which day; the
settings and the showing belong to each device and are never synced.

- **Android:** the coming reminders (at most 50, up to two months ahead) are handed to
  the system, and the whole set is replaced whenever the ledger or the settings
  change. They may arrive late: exact alarms are not asked for.
  - They go through the notification plugin's `batch` command. Its `notify` records
    nothing, so what it schedules cannot be cancelled or set again after a restart.
  - Scheduled is not delivered: one still pending an hour after its time was dropped
    (a force stop does this) and is shown when the app is next opened.
  - `scripts/build.sh` also sets them again after the app is updated.
- **Computers:** there is no scheduling, so they are shown while the app is open.
- Amounts are left out unless switched on, because a locked screen shows them.

## "Connect with loan apps"

Other Android apps do not expose their data, and there is no public interface a
personal app can use to read it. What TourLedger does instead: you paste a bank or
lender message and it drafts the payment for you to confirm, or you import a CSV.
It does not request permission to read your SMS inbox.

## Predictions

Plain statistics, not machine learning, and always labelled as estimates: a
weighted average of recent months per category. Loan dues are shown only in Loans. With fewer than two months of history it says so instead of guessing.

## UI performance

- Glass (`backdrop-filter`) only on fixed surfaces: navigation, header, sheets.
  Never on scrolling rows, which stutters on mid-range Android.
- Animate only `transform` and `opacity`, so motion follows the display's refresh rate.
- A "Reduce effects" switch, plus the system's reduce-motion setting.
- One layout: sidebar on wide screens, bottom bar on phones.
