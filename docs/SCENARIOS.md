# Scenarios the app must handle

Status: ✔ covered in v1 · ◻ not in v1

## Tours and events
- ✔ Company event (e.g. India International Trade Fair, ITPO, Bharat Mandapam, New Delhi)
- ✔ Personal trip with no company
- ✔ Company wellness trip, reward or bonus trip (spending not claimed unless marked)
- ✔ Several employers over time; one shown at a time
- ✔ Typing expenses as plain lines, checked before saving
- ✔ Daily personal expense that belongs to no tour
- ✔ Travel with regular colleagues or partners, saved once and reused
- ✔ Repeat journeys (home → railway station by auto) saved as quick entries

## Who paid
- ✔ I paid, company reimburses
- ✔ I paid, my own cost
- ✔ Company booked and paid (rail or air tickets): in tour cost, not in claim
- ✔ Colleague paid: in tour cost; my share shows as owed to them
- ✔ I paid for the group on a personal trip: split equally, shows who owes me

## Advances and claims
- ✔ Advance smaller than spend → company owes me
- ✔ Advance larger than spend → I return the balance
- ✔ Several advances, partial reimbursements, advance returned
- ✔ A claim line rejected by the company moves to my own cost
- ✔ Claim report by category as PDF and CSV
- ✔ Bonus or allowance kept, not set against the claim
- ✔ Foreign currency expense with its own exchange rate
- ✔ Bill photos and screenshots on expenses, in the claim PDF four to a page
- ✔ Preview of every page before saving
- ◻ PDF invoices as bills
- ◻ Per-diem and policy limit rules

## Loans and money
- ✔ EMI loan with amortisation schedule, paid/unpaid instalments, next due
- ✔ Part prepayment
- ✔ Many loans running and closed; close, reopen, mark months paid or unpaid
- ✔ Money lent to or borrowed from a person, repaid in pieces
- ✔ Draft a payment from a pasted bank message
- ◻ Automatic reading of other apps (not possible; see ARCHITECTURE.md)

## Planning and savings
- ✔ Monthly income and budget, savings estimate
- ✔ Spend by category and month, month-over-month change
- ✔ "Last year this month you spent …" reminder
- ✔ Reminders for unsettled claims (Home) and EMI dues (Loans only)
- ◻ System notifications while the app is closed

## Devices
- ◻ Mac ↔ Android ↔ Windows ↔ Linux sync: built, tested only between two processes on one Mac; no phone has linked yet
- ◻ Local network permission on Android 17: built, not tried on a phone
- ✔ Turn sync off, unlink one device, keep a tour on one device only
- ✔ Offline entry on several devices, merged later
- ✔ Lost device: unlink it on every other device so it receives nothing further (its existing copy cannot be erased remotely)
- ◻ One-step removal of a lost device from all devices at once
- ✔ Backup and restore through the sync file
- ◻ iOS, FreeBSD: recipes only (docs/PLATFORMS.md)
