[![Rust](https://github.com/harisr92/accounting-tui/actions/workflows/rust.yml/badge.svg)](https://github.com/harisr92/accounting-tui/actions/workflows/rust.yml) 
[![Package macOS](https://github.com/harisr92/accounting-tui/actions/workflows/package-macos.yml/badge.svg)](https://github.com/harisr92/accounting-tui/actions/workflows/package-macos.yml)
[![Package Linux](https://github.com/harisr92/accounting-tui/actions/workflows/package-linux.yml/badge.svg)](https://github.com/harisr92/accounting-tui/actions/workflows/package-linux.yml)
[![Package Windows](https://github.com/harisr92/accounting-tui/actions/workflows/package-windows.yml/badge.svg)](https://github.com/harisr92/accounting-tui/actions/workflows/package-windows.yml)
# compter

A terminal app for freelancers who work for several clients, billed by the hour or per feature. Track work as **projects** split into **milestones**, raise GST invoices from milestones, record payments, and get the numbers you need to file income tax and GST returns, all without leaving the terminal.

Built with [ratatui](https://ratatui.rs) (crossterm backend) on top of [`accounting-core`](https://github.com/harisr92/accounting-blue), which provides double-entry bookkeeping, GST invoices with PDF export, GSTR-1 and financial reports.

```
┌ compter ─────────────────────────────────────────────────────────────────── FY 2026-27 ┐
│ 1 Dashboard │ 2 Projects │ 3 Invoices │ 4 Reports                                      │
└────────────────────────────────────────────────────────────────────────────────────────┘
┌ Projects ─────────────────┐┌ Website Revamp ─────────────────────────────────── ACTIVE ┐
│> Website Revamp  ACTIVE   ││Client Acme Pvt Ltd   GSTIN 29ABCDE1234F1Z5                │
│  Mobile App      ACTIVE   ││Total               Paid               Outstanding         │
│  API Audit       PAID     ││₹1,50,000.00        ₹50,000.00         ₹1,00,000.00        │
│  Data Pipeline   COMPLETED│└───────────────────────────────────────────────────────────┘
│                           │┌ Milestones ───────────────────────────────────────────────┐
│                           ││  Name               Amount          Due         Status    │
│                           ││> Design                 ₹50,000.00  2026-09-01  PAID      │
│                           ││  Frontend               ₹60,000.00  2026-10-20  INVOICED  │
│                           ││  Launch                 ₹40,000.00  2026-11-15  PENDING   │
└───────────────────────────┘└───────────────────────────────────────────────────────────┘
Tab focus · j/k move · n new · i invoice · 1-9/←→ tabs · ? help · q quit
```

## Status

**All 7 steps are done.** Track projects, milestones and hours, and archive finished projects. Issue GST tax invoices, export invoices under LUT or plain invoices as PDFs, record payments including TDS, and correct invoices with credit notes. Record and edit business expenses. See each financial year for income tax (44ADA and regular books) and GST, and export the quarterly GSTR-1 and the monthly IFF. Search the project, invoice and expense lists, and pick a dark, light or monochrome theme. Everything is posted to an accounting-core ledger and saved as you go.

## Build and run

```bash
cargo run              # launch compter on your data
cargo run -- --demo    # try it on sample projects kept in memory; nothing is saved
cargo test             # unit, view and storage tests
cargo clippy --all-targets -- -D warnings
```

Only one compter can have the database open at a time; a second one exits with a clear message.

Prebuilt binaries are attached to each [GitHub release](../../releases): macOS (arm64, x86_64, universal), Linux (static, x86_64 and arm64) and Windows (x86_64 and arm64), each with a `SHA256SUMS-<platform>.txt`.

Needs Rust 1.88+ (the `pdf` feature of `accounting-core`) and a terminal with Unicode support. Runs on macOS, Linux and Windows. `o` opens PDFs with `open`, `xdg-open` or `start`; set `COMPTER_OPEN` to use another program.

## Key bindings

**Everywhere**

| Key | Action |
|-----|--------|
| `1`–`9`, `←`/`→` (`h`/`l`) | switch tab |
| `?` | help popup with every binding (`Esc` closes it) |
| `q` | quit |
| `Ctrl-c` | quit, even while a form is open |

**Projects tab**

| Focus | Key | Action |
|-------|-----|--------|
| any | `j`/`k`, `↓`/`↑` | move the selection (wraps around) |
| any | `Tab` | switch between the project list and milestones (`Enter` on a project also works) |
| any | `n` | new project |
| project list | `e` | edit name, client, GSTIN, billing or hourly rate |
| project list | `x` | delete the project (asks first; only while nothing in it is invoiced) |
| project list | `A` | archive a finished project (nothing pending), or restore an archived one |
| project list | `H` | show or hide archived projects (hidden by default; their names are dimmed) |
| project list | `/` | search projects by name or client; `Esc` clears the search |
| milestones | `a` | add a milestone |
| milestones | `e` / `x` | edit / delete a pending milestone (delete asks first) |
| milestones | `t` | log time on an hourly milestone |
| milestones | `o` | open or close the milestone's time log: its entries (date, note, time) are listed under it |
| milestones | `u` | undo the last time entry (asks first) |
| milestones | `i` | invoice a pending milestone: shows the number, total and tax, then asks |
| milestones | `p` | record a payment, whole or part, for an invoiced milestone |

**Invoices tab**: `j`/`k` move, `o` opens the PDF in your viewer (it's regenerated if the file is missing), `p` records a payment, whole or part, `c` issues a credit note (see [Credit notes](#credit-notes)), `O` opens the latest credit note's PDF, and `/` searches by number, client, milestone, kind or status (`Esc` clears it). The list shows each invoice's amount due and status (ISSUED, PART PAID, PAID, CREDITED); below it, the selected invoice's payments and credit notes. The title shows the total still outstanding.

**Dashboard tab**: this financial year's invoiced (net of credit notes), credit notes, received, outstanding, TDS, expenses and books profit (the tiles wrap onto two rows on narrow screens), plus every unpaid invoice and how long it has been waiting (red after 30 days).

**Expenses tab**: `a` adds an expense (date, category, description, amount paid including GST, and the GST you claim back as input credit), `e` edits one (its ledger posting is replaced in the same write), `x` deletes one (asks first), `/` searches by description or category, `[`/`]` change the year.

**Reports tab**: `[`/`]` change the year, `g` exports the GSTR-1 for a quarter, `i` exports a month's IFF. See [Reports](#reports).

**Settings tab**: `e` edits your business details: name, address, GSTIN, SAC code, invoice prefix, LUT ARN, and terms such as bank details. They must be filled in before the first invoice. `l` sets a logo (path to a PNG or JPEG under 1 MB; blank removes it), printed at the top left of new invoices. `t` switches the theme: dark, light (for light terminal backgrounds) or monochrome. The choice is saved. `s` opens a folder browser to pick the folder this device syncs through, and `x` stops syncing (see [Using several devices](#using-several-devices)). With [`NO_COLOR`](https://no-color.org) set, every theme is drawn without colour.

**Forms**: type to edit, `Backspace` deletes, `Ctrl-u` clears the field. `Tab`/`↓` and `Shift-Tab`/`↑` move between fields, and `Space`/`←→` change a choice. `Enter` saves and `Esc` cancels. If something is wrong, the form stays open with the reason shown in red and your input kept.

**What you can type**

| Field | Accepts |
|-------|---------|
| amounts | `50000`, `1,50,000.50`, `₹1,50,000` |
| dates | `2026-10-31`; blank means today |
| time | `1h30m`, `2h`, `45m`, `1:30`, or a plain number of **hours** (`1.5` = 1h 30m). The field shows what it will log as you type (`= 1h 30m`). One entry can be at most 24 hours, so `90` is refused rather than logged as 90 hours. |
| GSTIN | 15 characters, any case; blank for an unregistered client |
| state code | 2-digit GST code for an unregistered Indian client (`29` = Karnataka; the field names the state as you type) |
| SAC code | must be in the GST rate list; the field shows its description and rate |
| payment | received + TDS can be any part of what is still due, never more. Received is prefilled with the balance and TDS with 0; the hint shows 10% of the taxable value |

An invoiced milestone is locked: it can't be edited, deleted or have time added. A project with any invoiced or paid milestone can't be deleted either, because those milestones are the billing records your tax and GST filings come from. Archive it instead: an archived project leaves the list but keeps its records, still counts in reports, and can still receive payments. New milestones need it restored first. Changing an hourly rate re-prices only pending milestones. Fixed and hourly billing can only be switched while a project has no milestones.

## Data model

```rust
pub struct Project {
    id: ProjectId,                  // UUID
    name: String,
    client_name: String,
    client_gstin: Option<Gstin>,    // accounting-core's validated GSTIN; None = unregistered
    billing: Billing,               // Fixed | Hourly { rate }
    milestones: Vec<Milestone>,
    total_amount: BigDecimal,       // derived: sum of milestone amounts
    status: ProjectStatus,          // derived: ACTIVE | COMPLETED | PAID
}

pub struct Milestone {
    name: String,
    amount: BigDecimal,             // fixed: agreed amount; hourly: derived from time
    due_date: NaiveDate,
    status: MilestoneStatus,        // PENDING | INVOICED | PAID | CREDITED
    credited: BigDecimal,           // taken back by credit notes, before GST
    time_entries: Vec<TimeEntry>,   // hourly only: date, minutes, note
}
```

Fields are private. All changes go through `Project` methods, which validate the input and then recompute the derived fields.

**Billing** is set per project:
- **Fixed** (feature-based): each milestone gets an agreed amount when it is added.
- **Hourly**: milestones are added without an amount. Logged time sets it: `rate × minutes / 60`, rounded half-up to paise. Time is stored as whole minutes, so the arithmetic stays exact.

**Milestone transitions**, one step at a time:

| From | To | Method | Rule |
|------|----|--------|------|
| PENDING | INVOICED | `invoice_milestone` | amount must be > 0 |
| INVOICED | PAID | `record_payment` | when payments (and any credit notes) settle the invoice |
| INVOICED | CREDITED | `cancel_milestone` | when credit notes cancel the whole invoice and nothing was paid |

Once a milestone is invoiced it is locked: no more time entries and it can't be removed. Any other transition returns `DomainError::InvalidTransition`.

**Project status** is derived from the milestones:

| Status | When |
|--------|------|
| ACTIVE | no milestones yet, or any milestone is pending |
| COMPLETED | none pending, at least one invoiced but unpaid |
| PAID | every milestone is paid or credited |

**Serialisation**: projects are serde types (JSON in step 3). Loading runs the same validation as the constructors (names, GSTIN, amounts, rate, billing consistency) and recomputes `total_amount`, `status` and the amounts of pending hourly milestones. Derived values in the file are never trusted. The exception is the amount of an invoiced or paid milestone: it is the billed record, so it is kept as stored (it must be > 0) and does not follow later rate changes.

The financial year is the Indian April–March year (`FY 2026-27`).

## Invoicing

`i` on a pending milestone issues one invoice for it, numbered `PREFIX/26-27/001` (≤ 16 characters, as Rule 46 requires). The sequence restarts every April and never skips or repeats. The kind is chosen for you:

| You | Client | Invoice | Tax |
|-----|--------|---------|-----|
| GST-registered | in India, registered | Tax invoice | CGST + SGST in your state, IGST across states (18% for SAC 998314) |
| GST-registered | in India, unregistered | Tax invoice | as above, by the client's state code (place of supply) |
| GST-registered | abroad | Tax invoice, export of services | zero-rated under your LUT, endorsed on the invoice |
| not registered | anyone | Invoice | none; the invoice says so |

- Tax invoices are built and checked by accounting-core (`GstInvoice`, `validate_invoice`). Compliance errors block issuing, and warnings appear in the confirmation.
- Unregistered clients need a name and address from ₹50,000, and clients abroad always need one.
- The PDF is rendered before anything is saved, so an invoice that can't be printed is never issued. It is written to `<data dir>/invoices/2026-27/PREFIX-26-27-001.pdf`.
- An invoice keeps its own copy of the printed document and terms, so its PDF comes out identical later, even after you change Settings. It also records which logo it was issued with: changing or removing the logo affects only new invoices.
- Issuing writes the invoice, the milestone's new status, the invoice counter and the ledger posting in **one transaction**.

**Ledger** (accounting-core, created on first use):

| Account | | Posted by |
|---------|-|-----------|
| 1100 Bank | asset | payments received |
| 1200 Accounts Receivable | asset | invoice totals, cleared by payments |
| 1250 TDS Receivable | asset | TDS clients withheld (claim it in your ITR) |
| 2300 / 2310 / 2320 CGST / SGST / IGST Output | liability | GST on tax invoices |
| 4100 Service Revenue | income | invoices to Indian clients |
| 4110 Export Revenue | income | invoices to clients abroad |
| 4190 Credit Notes | income (contra) | value taken back by credit notes |
| 1300 Input GST | asset | GST on expenses claimed as input credit |
| 5100–5190 expense accounts | expense | one per category: software, equipment, internet & phone, travel, rent & coworking, professional fees, other |

**Payments** can come in parts. Each payment records the amount that reached your bank and any TDS the client withheld on it, and is posted to the ledger on its own (`pay-INV/26-27/004-1`, `-2`, …). An invoice is ISSUED until the first payment, PART PAID while a balance remains, and PAID when the payments add up to its total. A payment larger than the balance is refused. The milestone turns PAID only with the final payment. Reports count each payment, and its TDS, on its own date.

## Credit notes

An issued invoice is never edited. To correct one, issue a **credit note** against it: `c` in the Invoices tab, then the date, the amount to take back **before GST**, and the reason. compter shows the note, with its GST and what will still be due, and asks before issuing it.

- Notes are numbered in their own series, `CN/26-27/001`, which restarts every April.
- On a tax invoice, accounting-core builds the note (`CreditNote`) at the invoice's GST rate and posts the invoice's legs reversed: Dr 4190 Credit Notes and the output GST, Cr Receivable. Notes on export and plain invoices carry no GST and post Dr 4190, Cr Receivable. A note on an invoice in a foreign currency is at the invoice's exchange rate.
- A note takes back only what is still due. It can't refund money already received.
- Under GST section 34(2), a note against a tax or export invoice must be issued by **30 November after the end of the invoice's financial year**: 30 November 2026 for invoices dated April 2025 to March 2026. Later notes are refused. If you file your annual return earlier, that date is the deadline instead, and compter can't know it.
- Each note is recorded on the invoice's milestone (`credited`, before GST), so the project's **Paid** counts only money received and its **Outstanding** drops by the credit. The project summary shows a **Credited** tile once any credit exists.
- A note that leaves nothing due settles the invoice and its milestone: CREDITED if nothing was paid, PAID otherwise.
- The PDF is written to `<data dir>/invoices/2026-27/CN-26-27-001.pdf`, rendered before anything is saved, like an invoice's. It prints the original invoice's number and date and the reason.
- Reports subtract a note in the year and quarter it is dated in: gross receipts, GST and the client's invoiced figure. GSTR-1 reports it in table 9B (`cdnr` for registered clients, `cdnur` against large B2CL invoices, `cdnur`/`EXPWOP` for exports) and in table 13.

**Known limitation**: accounting-core's PDF layout always prints a GST supply line ("Intra-state (CGST + SGST)") and a seller GSTIN line, even on a plain invoice. compter adds "no GST is charged on this invoice" as the first term so the document is unambiguous.

## Reports

The Reports tab shows one financial year (April–March). All figures are computed when you open it, from the invoices and expenses on record:

| Section | What it shows |
|---------|---------------|
| Income | invoices dated in the year; **gross receipts** (their value excl. GST, less credit notes dated in the year) and how much of that is exports; money **received** and **TDS** withheld on payments dated in the year; what is still **outstanding** |
| Income tax | **44ADA income**, 50% of gross receipts, next to **books profit** (gross receipts − expenses). It flags receipts over the ₹75 lakh 44ADA limit, and a books profit below 44ADA income, since declaring the lower figure needs a tax audit (44AB) when income is over the basic exemption |
| GST | per quarter: CGST, SGST and IGST charged, input credit from expenses, and the net payable (or credit carried forward) |
| Clients | per client: invoiced, received and TDS, to match against Form 26AS |

compter doesn't calculate the tax you owe: slab rates change every budget.

Assumptions:
- **Gross receipts** count invoices by invoice date (accrual).
- **TDS** is counted on the payment date. A client may deposit TDS in an earlier quarter, at the time they booked your invoice, so check against Form 26AS.

**GSTR-1 (quarterly, QRMP)**: `g`, pick the quarter, and compter writes the GST portal JSON to `<data dir>/returns/2026-27/GSTR1-Q3-2026-27.json` with the quarter's last month as the return period. Upload it with the GST offline tool or the portal.
- accounting-core builds each month's return (B2B, B2CL, B2CS, nil, HSN). compter merges the three months, then adds:
  - **6A exports** under LUT (`WOPAY`), which accounting-core doesn't cover
  - **Table 13**, the documents-issued summary, covering tax and export invoices
- Plain invoices are not reported: they are issued while unregistered.
- Exports are added to the HSN summary's B2C section at 0%. **Confirm this against the current portal rules before filing.** The shipping-bill fields are left out, since services have no shipping bill.
- Credit notes dated in the quarter go into table 9B and table 13 (see [Credit notes](#credit-notes)).
- **IFF** filed for the first or second month? Say so in the export form ("IFF filed"), and those months' B2B invoices and B2B credit notes are left out of the quarter's return. They still count in its HSN summary and table 13.

**IFF (monthly, QRMP)**: `i`, pick one of a quarter's first two months, and compter writes that month's B2B invoices (table 4A) and credit notes on B2B invoices (9B `cdnr`) to `<data dir>/returns/2026-27/IFF-2026-07.json`. Then answer "IFF filed" when you export that quarter's GSTR-1.

## Architecture

```
            ┌──────────┐   keys / ticks   ┌───────────┐
 terminal ──│ event.rs │ ───────────────▶ │  app.rs   │  state, global keys, dirty flag
            └──────────┘  bounded channel └─────┬─────┘
                                                │ &App
                                          ┌─────▼─────┐
                                          │    ui/    │  header, footer, popup, views (View trait)
                                          └─────┬─────┘
                                                │ reads
   ┌────────────┐      ┌────────────┐     ┌─────▼─────┐
   │  storage/  │ ───▶ │ services/  │ ──▶ │  domain/  │  Project, Milestone, Billing, DomainError
   │   (redb)   │      │ (step 5+)  │     └───────────┘
   └────────────┘      └─────┬──────┘
                             ▼
                      accounting-core  (GstInvoice, Ledger, PDF, GSTR-1, reports)
```

```
crates/compter-core/src/  the `compter_core` crate: no UI, shared by every front end
├── lib.rs               library root
├── demo.rs              in-memory sample business for --demo; fixture projects for tests
├── services/            use-cases: load → domain change → save
│   ├── project_service.rs  ProjectService, ProjectDraft, MilestoneDraft
│   ├── invoice_service.rs  InvoiceService: profile, preview, issue, pay, credit, PDFs
│   ├── preferences_service.rs  PreferencesService: the saved theme
│   ├── invoice_builder.rs  milestone → invoice document + ledger entries (pure)
│   ├── credit_note_builder.rs  correction → credit note document + ledger entries (pure)
│   ├── expense_service.rs  ExpenseService: add, edit and delete expenses with their posting
│   ├── reports.rs       YearReport: a financial year in numbers (pure)
│   ├── report_service.rs  ReportService: year reports, GSTR-1 and IFF exports
│   ├── gstr1.rs         quarterly GSTR-1: merge months, exports (6A), export credit notes, table 13; monthly IFF
│   ├── chart.rs         compter's ledger accounts
│   └── error.rs         ServiceError
├── storage/             redb persistence
│   ├── store.rs         Store: open/in-memory, schema check, hands out the repository
│   ├── repository.rs    Project/Invoice/Settings repository traits, IssueRecord, PaymentRecord
│   ├── redb_repository.rs  RedbRepository: all three traits; issue/payment in one transaction
│   ├── ledger.rs        RedbLedgerStorage (accounting-core AccountStore + TransactionStore)
│   ├── tables.rs        every table definition
│   ├── codec.rs         JSON encode/decode of values
│   ├── migrations.rs    upgrade steps between schema versions
│   ├── files.rs         atomic file writes (invoice PDFs)
│   ├── data_dir.rs      where compter.redb and invoices/ live
│   ├── sync.rs          hand the database between devices through a synced folder
│   └── error.rs         StorageError
├── domain/              pure types and rules
│   ├── project.rs       Project, ClientDetails/ClientRegion, ProjectStatus, mutations
│   ├── milestone.rs     Milestone, MilestoneId, MilestoneStatus, transitions
│   ├── billing.rs       Billing, TimeEntry, hourly amount calculation
│   ├── invoice.rs       Invoice, InvoiceNumber, InvoiceKind, InvoiceStatus, payment and credit rules
│   ├── credit_note.rs   CreditNote: a correction to an issued invoice
│   ├── appearance.rs    Appearance: the chosen colour theme
│   ├── profile.rs       BusinessProfile (seller details, GSTIN, SAC, prefix, LUT)
│   ├── expense.rs       Expense, ExpenseCategory, ExpenseId
│   ├── states.rs        GST state code names
│   ├── error.rs         DomainError
│   └── fiscal_year.rs   Indian April–March year and its quarters
└── (tests/ beside each module; database-file integration tests in crates/compter-core/tests/)

crates/compter-tui/src/   the `compter_tui` crate: the terminal app
├── main.rs              binary: args, panic hook, store, six tabs, event loop
├── lib.rs               library root
├── app.rs               App state, global keys, tab switching (on_activate)
├── event.rs             crossterm input, resize and tick threads
├── cli.rs               import-merges and sync commands
├── viewer.rs            open a PDF in the desktop's default app
└── ui/
    ├── mod.rs           draw(): header / body / footer, help overlay
    ├── view.rs          View trait (render, keys, hints, on_activate), Action, Notice
    ├── theme.rs         every colour and style: dark, light and monochrome
    ├── filter.rs        Filter: the `/` search shared by the list tabs
    ├── widgets.rs       panel, modal, money, duration, status_span
    ├── selection.rs     wrap-around Selection and j/k Nav
    ├── form/            reusable modal Form (Form::step), Confirm, input parsers
    ├── header.rs  footer.rs  popup.rs
    └── views/
        ├── projects/    ProjectsView: state and keys, rendering, forms, issue/pay
        ├── invoices/    InvoicesView: list, search, open PDF, record payment, credit notes
        ├── settings/    SettingsView: business profile, logo, theme and sync folder
        ├── payment.rs   the payment form shared by Projects and Invoices
        ├── dashboard/   DashboardView: this year and unpaid invoices
        ├── expenses/    ExpensesView: add, edit, delete, search, by year
        ├── reports/     ReportsView: income, 44ADA vs books, GST, clients, GSTR-1 and IFF export
        ├── year.rs      YearPicker: [ and ] through financial years
        └── placeholder.rs  a tab that only shows a message
```

Unit tests sit in a `tests/` folder beside the code they cover (`domain/tests/`, `storage/tests/`, `services/tests/` in compter-core; `ui/tests/` and `tests/` for `app.rs` in compter-tui), one `<file>_tests.rs` per source file. Integration tests that reopen a real database file are in `crates/compter-core/tests/`.

Each tab implements `View`. A new screen is a new `View` registered in `main.rs`; the core loop doesn't change. See `CLAUDE.md` for the SOLID, DRY and memory rules the code follows.

## Storage

Everything lives in one [redb](https://www.redb.org) database file, `compter.redb`. redb is an embedded, ACID key-value store. The file goes in the first of these that is set:

1. `$COMPTER_HOME`
2. `$XDG_DATA_HOME/compter`
3. `%APPDATA%\compter` on Windows (whichever shell started compter), `$HOME/.local/share/compter` elsewhere

| Table | Key | Value |
|-------|-----|-------|
| `meta` | `"schema_version"` | `3` |
| `projects` | project UUID | full project with milestones and time entries |
| `project_summaries` | project UUID | id, name, client, status, total, archived, for list views |
| `accounts` | account id | accounting-core `Account` |
| `transactions` | transaction id | accounting-core `Transaction` (`inv-…` / `pay-…` per invoice, `cn-…` per credit note) |
| `invoices` | invoice number | issued invoice with its printed document, its payments and its credit notes |
| `invoice_summaries` | invoice number | number, date, client, total, balance, status, for the list |
| `invoice_counters` | financial year (`2026-27`) | last sequence used |
| `credit_notes` | credit note number | issued credit note with its printed document |
| `credit_note_counters` | financial year (`2026-27`) | last credit note sequence used |
| `settings` | `"business_profile"`, `"appearance"` | your business details; the theme |
| `expenses` | expense UUID | an expense (its posting is transaction `exp-<id>`) |
| `logos` | logo UUID | raw PNG or JPEG bytes; never removed, since invoices refer to them |

- **Atomic**: a project and its summary are written in one transaction, and issuing or paying an invoice writes every affected table in one transaction. The lists can never disagree with the records, and a crash leaves the last committed state.
- **Low memory**: nothing is cached. Lists read only the small summary rows; a full project is read when you open it. The ledger store answers each query from the database.
- **Validated on load**: values are stored as JSON bytes and decoded through the same checks as the domain constructors. A hand-edited bad record fails with its table and key instead of loading.
- **Single process**: redb locks the file, so a second `compter` is refused with a clear error instead of corrupting data.
- **Versioned, with upgrades**: the `meta` table records the schema version. Opening an older database runs the upgrade steps in `storage/migrations.rs` within one transaction. A database from a *newer* compter is refused rather than guessed at. Saved-format fixtures (`storage/tests/fixtures/`) are loaded on every `cargo test`, so a change that would stop existing records from loading fails the build first. Version 2 turned an invoice's single paid status into a list of payments; a version 1 database is upgraded on first open. Version 3 added credit notes and the CREDITED milestone status; it rewrites nothing, but a compter from before it refuses the file instead of reading a credited invoice as still owed in full.

`Store::open(&DataDir)` opens the file, and `Store::in_memory()` gives the same database in memory for tests. From a store you get:
- `store.repository()`: a `RedbRepository`, which implements `ProjectRepository`, `InvoiceRepository` and `SettingsRepository`
- `store.ledger()`: an accounting-core `AccountStore` + `TransactionStore`, so `Ledger::new(store.ledger())` works directly

The app opens the file on start, before the terminal is taken over, so an error like "already open" prints normally.

### Using several devices

Don't put `compter.redb` itself in Google Drive, OneDrive, Dropbox or iCloud. redb's lock only works on one machine, a sync client can upload the file halfway through a write, and edits made on two devices become a "conflict copy" that silently drops one side. Invoice numbers have to stay sequential, so two devices' changes can't be merged either.

Instead, on every device, open **Settings** and press `s` to pick a synced folder, e.g. `~/Library/CloudStorage/GoogleDrive-you@example.com/My Drive/compter-sync`. The browser opens in the current sync folder (or your home folder): `j`/`k` move, `Enter` opens a folder, `Backspace` goes up, `~` jumps home, `.` shows hidden folders, and `Space` chooses the folder shown. It must exist and be apart from the data folder. The choice belongs to the device (each one may see the folder at a different path), so it is saved in `device.json` in the data folder, not in the database, and takes effect the next time compter starts. Press `x` to stop syncing.

`COMPTER_SYNC_DIR`, when set, overrides that setting, which is handy for scripts:

```bash
export COMPTER_SYNC_DIR="$HOME/Library/CloudStorage/GoogleDrive-you@example.com/My Drive/compter-sync"
```

Each device keeps its own database in its data folder. compter hands it over through the synced folder:

- **On exit**, if the records changed, compter copies the database to `compter.redb` in the synced folder, then writes `manifest.json`, which numbers the snapshot and records its SHA-256. Opening compter without changing anything publishes nothing.
- **On start**, if the manifest is newer than what this device last synced, compter pulls in the snapshot. It does this only when the local copy is unchanged since then, and only once the snapshot's hash matches the manifest, so a half-synced file is never used.
- **While open**, `lease.json` names the device using compter and is refreshed every minute. Another device refuses to start while it's fresh, and takes it over after 5 minutes (a crash). Name devices with `COMPTER_DEVICE`; the host name is the default.

**Quit compter on one device, and wait for the sync client, before opening it on another.** If both devices changed the data anyway, compter refuses to open and tells you. Settle it on the device you are at:

- `compter sync --keep-remote` replaces this device's data with the snapshot, and keeps the old file as `compter.conflict-<time>.redb`.
- `compter sync --keep-local` publishes this device's data as the newest snapshot.

Conflict copies made by the sync client (`compter (1).redb` and the like) are never read; compter shows a warning in the footer when it finds them. Invoice PDFs and returns stay in each device's data folder.

### Phones

With the synced folder in Google Drive, the phone app syncs with it automatically: **Settings → Sync method → Google Drive**, then pick the folder. It takes the computer's newer snapshot whenever you open the app, sends its changes every minute and when you put it away, and holds a lease while open, so compter on the computer waits for it (and the other way round). Setting up the Google sign-in it needs is described in `app/README.md`.

Otherwise you carry the two files across by hand, in **Settings → Sync with your computer**. Quit compter on the computer first, and let the sync client finish.

- **Get from computer** picks `compter.redb` and `manifest.json` from the synced folder (Files on iPhone, the file picker on Android, both showing iCloud Drive and Google Drive). A new install simply takes them. If the phone changed since its last hand-off, it asks which side to keep, as `compter sync` does.
- **Send to computer** writes the phone's snapshot and manifest and opens the share sheet: save both into the synced folder, **replacing** the ones there. On iPhone, Files asks to replace them. The Google Drive app on Android uploads new files instead (`compter (1).redb`); then replace the originals with them from the computer.

A hand-off that never reached the folder is caught: the next **Get from computer** finds a snapshot other than the one the phone sent, and asks which side to keep instead of taking or overwriting either. The computer, for its part, warns about the copies.

## Roadmap

1. ✅ **TUI layout shell**: tabs, project list and milestone table, footer hints, help popup
2. ✅ **Domain model**: serde, hourly billing and time entries, status transitions, validation
3. ✅ **Storage**: redb database, project repository, ledger storage for `accounting-core`
4. ✅ **Wire storage into the TUI**: load and save, forms to create and edit projects and milestones, log hours
5. ✅ **Invoicing & payments**: tax, export (LUT) and plain invoices, PDFs, ledger posting, payments with TDS, Invoices and Settings tabs
6. ✅ **Tax and reports**: dashboard, expenses, year report (income, 44ADA vs books, GST by quarter, TDS and income by client), quarterly GSTR-1 JSON with exports
7. ✅ **Polish**: archive finished projects (hidden from the list, kept for tax records), credit notes, editing expenses, IFF, search and filter, theming
