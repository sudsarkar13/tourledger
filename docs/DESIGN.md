# Design rules

One place for values: `ui/src/styles/theme.css`.

## Two separate areas

| Area | Pages | Shows loan data? |
|---|---|---|
| Tours and expenses | Home · Tours · Tour · Claim report · Reports | **Never** |
| Loans | Dashboard · My loans · Loan · EMI tracker · Insights | Only here |

Home answers three questions about tours: what did I spend, what has the company
given me, what is still to come back. No loan figure, reminder or total appears
outside the Loans area, with one exception: More → Notifications lists the names
and due dates of coming instalments, without amounts, to show what is set.

## Companies

A person's employment details (designation, employee ID, department, dates) belong
to a company, not to the profile, so changing jobs keeps old claims correct. The
company switcher on Home, Tours and Reports shows **one company, or personal
trips, never mixed**. It starts on the current company and is remembered on the
device. Setting up (profile, companies, colleagues) lives under More, not Home.
Dropdown arrows are drawn by the app, set in from the edge.

## Page shape

```
AppShell               sidebar (≥ 840px) or bottom bar: Home Tours Reports Loans More
  PageHeader           title (h1), one line saying what the page is for, actions
  Tabs                 Loans area only
  content              sections, each a Card with an h2
Sheet                  every create and edit form; bottom sheet on phones
```

## Shared components (`ui/src/components`)

Every drop-down is `Select`: a button that opens a glass menu, floating beside it on a
wide screen and rising from the bottom on a phone. Give an option an `icon` where one
helps to tell choices apart. Never write a native `<select>`.

Button · Field · DateField · Select · Segmented · Tabs · Card · Sheet · Money · Stat ·
EmptyState · ListRow · Toast · Confirm · MonthBars · CategoryBars · Meter

A page never styles its own button, input or dialog.

## Updating

More → Updates holds the version, the state in one line, one primary button for the
next step, and the switch for automatic checks. A ready update asks once per version
in a `Sheet`, never over an open form or while something is being typed.

## Notifications

Only loan instalments are notified, and only after the person switches it on in
More → Notifications, which is also when permission is asked. A test button shows
whether it works.

## Behaviour

- Easy entry: a text box takes one expense per line, shows what it understood, and
  saves only after the person confirms. Rules, not AI. The full form is always
  available.
- Every number has a label saying what it counts.
- A claim lists totals by category unless "Every entry" is chosen; item names,
  dates and payers appear only in the detailed form. It names me by my profile
  name, and shows "Both" when two travellers shared a cost.
- A claim has no signature or approval boxes: it is only submitted.
- Dates are always `DateField`: a large calendar with month and year choice, the
  tour's days marked. Never the system date input.
- A journey can save its return in the same step: a second entry with From and To
  swapped, its own fare and date.
- A new expense's category follows its note until picked by hand. A saved
  category is only changed when the person presses a button to do it.
- Forms validate on submit; the message shows under the field, in words that say
  what to enter. Submit is disabled only while saving.
- A refused save shows its reason inside the form. Any other failed call shows a
  Toast with the reason from Rust.
- Deleting, and any action that changes many entries at once, asks through
  `Confirm`.
- Every list has an EmptyState that says what to do next.
- Motion: the launch screen stays at least 0.7 s; pages arrive block by block,
  from the right going deeper and from the left going back. Durations come from
  `theme.css` and are zero under "Reduce effects" or the system's reduced motion.
- Sync always says where it stands in words (off, linking, syncing with whom,
  waiting, up to date) and what the last exchange moved.
- Touch targets at least 44px; visible focus ring; every input has a label.
