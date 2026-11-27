# Design rules

One place for values: `ui/src/styles/theme.css`.

## Two separate areas

| Area | Pages | Shows loan data? |
|---|---|---|
| Tours and expenses | Home · Tours · Tour · Claim report · Reports | **Never** |
| Loans | Dashboard · My loans · Loan · EMI tracker · Insights | Only here |

Home answers three questions about tours: what did I spend, what has the company
given me, what is still to come back. No loan figure, reminder or total appears
outside the Loans area.

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

Button · Field · DateField · Select · Segmented · Tabs · Card · Sheet · Money · Stat ·
EmptyState · ListRow · Toast · Confirm · MonthBars · CategoryBars · Meter

A page never styles its own button, input or dialog.

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
