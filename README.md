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

**Steps 1–6 of 7 are done.** Track projects, milestones and hours. Issue GST tax invoices, export invoices under LUT or plain invoices as PDFs, and record payments including TDS. Record business expenses. See each financial year for income tax (44ADA and regular books) and GST, and export the quarterly GSTR-1. Everything is posted to an accounting-core ledger and saved as you go.

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
| milestones | `a` | add a milestone |
| milestones | `e` / `x` | edit / delete a pending milestone (delete asks first) |
| milestones | `t` | log time on an hourly milestone |
| milestones | `o` | open or close the milestone's time log: its entries (date, note, time) are listed under it |
| milestones | `u` | undo the last time entry (asks first) |
| milestones | `i` | invoice a pending milestone: shows the number, total and tax, then asks |
| milestones | `p` | record a payment, whole or part, for an invoiced milestone |

**Invoices tab**: `j`/`k` move, `o` opens the PDF in your viewer (it's regenerated if the file is missing), `p` records a payment, whole or part. The list shows each invoice's amount due and status (ISSUED, PART PAID, PAID); below it, the selected invoice's payments. The title shows the total still outstanding.

**Dashboard tab**: this financial year's invoiced, received, outstanding, TDS, expenses and books profit, plus every unpaid invoice and how long it has been waiting (red after 30 days).

**Expenses tab**: `a` adds an expense (date, category, description, amount paid including GST, and the GST you claim back as input credit), `x` deletes one (asks first), `[`/`]` change the year.

**Reports tab**: `[`/`]` change the year, `g` exports the GSTR-1 for a quarter. See [Reports](#reports).

**Settings tab**: `e` edits your business details: name, address, GSTIN, SAC code, invoice prefix, LUT ARN, and terms such as bank details. They must be filled in before the first invoice.

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

An invoiced milestone is locked: it can't be edited, deleted or have time added. A project with any invoiced or paid milestone can't be deleted either, because those milestones are the billing records your tax and GST filings come from. Changing an hourly rate re-prices only pending milestones. Fixed and hourly billing can only be switched while a project has no milestones.

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
    status: MilestoneStatus,        // PENDING | INVOICED | PAID
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
| INVOICED | PAID | `record_payment` | when the invoice's payments add up to its total |

Once a milestone is invoiced it is locked: no more time entries and it can't be removed. Any other transition returns `DomainError::InvalidTransition`.

**Project status** is derived from the milestones:

| Status | When |
|--------|------|
| ACTIVE | no milestones yet, or any milestone is pending |
| COMPLETED | none pending, at least one invoiced but unpaid |
| PAID | every milestone is paid |

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
- An invoice keeps its own copy of the printed document and terms, so its PDF comes out identical later, even after you change Settings.
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
| 1300 Input GST | asset | GST on expenses claimed as input credit |
| 5100–5190 expense accounts | expense | one per category: software, equipment, internet & phone, travel, rent & coworking, professional fees, other |

**Payments** can come in parts. Each payment records the amount that reached your bank and any TDS the client withheld on it, and is posted to the ledger on its own (`pay-INV/26-27/004-1`, `-2`, …). An invoice is ISSUED until the first payment, PART PAID while a balance remains, and PAID when the payments add up to its total. A payment larger than the balance is refused. The milestone turns PAID only with the final payment. Reports count each payment, and its TDS, on its own date.

**Known limitation**: accounting-core's PDF layout always prints a GST supply line ("Intra-state (CGST + SGST)") and a seller GSTIN line, even on a plain invoice. compter adds "no GST is charged on this invoice" as the first term so the document is unambiguous.

## Reports

The Reports tab shows one financial year (April–March). All figures are computed when you open it, from the invoices and expenses on record:

| Section | What it shows |
|---------|---------------|
| Income | invoices dated in the year; **gross receipts** (their value excl. GST) and how much of that is exports; money **received** and **TDS** withheld on payments dated in the year; what is still **outstanding** |
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
- Monthly IFF uploads for B2B invoices aren't generated. Everything is in the quarterly return.

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
src/
├── main.rs              binary: args, panic hook, store, six tabs, event loop
├── lib.rs               library root; everything below is the `compter_tui` crate
├── app.rs               App state, global keys, tab switching (on_activate)
├── event.rs             crossterm input, resize and tick threads
├── demo.rs              in-memory sample business for --demo; fixture projects for tests
├── services/            use-cases: load → domain change → save
│   ├── project_service.rs  ProjectService, ProjectDraft, MilestoneDraft
│   ├── invoice_service.rs  InvoiceService: profile, preview, issue, pay, PDFs
│   ├── invoice_builder.rs  milestone → invoice document + ledger entries (pure)
│   ├── expense_service.rs  ExpenseService: add and delete expenses with their posting
│   ├── reports.rs       YearReport: a financial year in numbers (pure)
│   ├── report_service.rs  ReportService: year reports, GSTR-1 export
│   ├── gstr1.rs         quarterly GSTR-1: merge months, exports (6A), table 13
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
│   └── error.rs         StorageError
├── domain/              pure types and rules
│   ├── project.rs       Project, ClientDetails/ClientRegion, ProjectStatus, mutations
│   ├── milestone.rs     Milestone, MilestoneId, MilestoneStatus, transitions
│   ├── billing.rs       Billing, TimeEntry, hourly amount calculation
│   ├── invoice.rs       Invoice, InvoiceNumber, InvoiceKind, InvoiceStatus, payment rules
│   ├── profile.rs       BusinessProfile (seller details, GSTIN, SAC, prefix, LUT)
│   ├── expense.rs       Expense, ExpenseCategory, ExpenseId
│   ├── states.rs        GST state code names
│   ├── error.rs         DomainError
│   └── fiscal_year.rs   Indian April–March year and its quarters
└── ui/
    ├── mod.rs           draw(): header / body / footer, help overlay
    ├── view.rs          View trait (render, keys, hints, on_activate), Action, Notice
    ├── theme.rs         every colour and style
    ├── widgets.rs       panel, modal, money, duration, status_span
    ├── selection.rs     wrap-around Selection and j/k Nav
    ├── form/            reusable modal Form (Form::step), Confirm, input parsers
    ├── header.rs  footer.rs  popup.rs
    └── views/
        ├── projects/    ProjectsView: state and keys, rendering, forms, issue/pay
        ├── invoices/    InvoicesView: list, open PDF, record payment
        ├── settings/    SettingsView: business profile
        ├── payment.rs   the payment form shared by Projects and Invoices
        ├── dashboard/   DashboardView: this year and unpaid invoices
        ├── expenses/    ExpensesView: add, delete, by year
        ├── reports/     ReportsView: income, 44ADA vs books, GST, clients, GSTR-1 export
        ├── year.rs      YearPicker: [ and ] through financial years
        └── placeholder.rs  a tab that only shows a message
```

Unit tests sit in a `tests/` folder beside the code they cover (`src/domain/tests/`, `src/storage/tests/`, `src/ui/tests/`, and `src/tests/` for `app.rs`), one `<file>_tests.rs` per source file. Integration tests that reopen a real database file are in the top-level `tests/` directory.

Each tab implements `View`. A new screen is a new `View` registered in `main.rs`; the core loop doesn't change. See `CLAUDE.md` for the SOLID, DRY and memory rules the code follows.

## Storage

Everything lives in one [redb](https://www.redb.org) database file, `compter.redb`. redb is an embedded, ACID key-value store. The file goes in the first of these that is set:

1. `$COMPTER_HOME`
2. `$XDG_DATA_HOME/compter`
3. `%APPDATA%\compter` on Windows (whichever shell started compter), `$HOME/.local/share/compter` elsewhere

| Table | Key | Value |
|-------|-----|-------|
| `meta` | `"schema_version"` | `2` |
| `projects` | project UUID | full project with milestones and time entries |
| `project_summaries` | project UUID | id, name, client, status, total, for list views |
| `accounts` | account id | accounting-core `Account` |
| `transactions` | transaction id | accounting-core `Transaction` (`inv-…` / `pay-…` per invoice) |
| `invoices` | invoice number | issued invoice with its printed document and its payments |
| `invoice_summaries` | invoice number | number, date, client, total, balance, status, for the list |
| `invoice_counters` | financial year (`2026-27`) | last sequence used |
| `settings` | `"business_profile"` | your business details |
| `expenses` | expense UUID | an expense (its posting is transaction `exp-<id>`) |

- **Atomic**: a project and its summary are written in one transaction, and issuing or paying an invoice writes every affected table in one transaction. The lists can never disagree with the records, and a crash leaves the last committed state.
- **Low memory**: nothing is cached. Lists read only the small summary rows; a full project is read when you open it. The ledger store answers each query from the database.
- **Validated on load**: values are stored as JSON bytes and decoded through the same checks as the domain constructors. A hand-edited bad record fails with its table and key instead of loading.
- **Single process**: redb locks the file, so a second `compter` is refused with a clear error instead of corrupting data.
- **Versioned, with upgrades**: the `meta` table records the schema version. Opening an older database runs the upgrade steps in `storage/migrations.rs` within one transaction. A database from a *newer* compter is refused rather than guessed at. Saved-format fixtures (`storage/tests/fixtures/`) are loaded on every `cargo test`, so a change that would stop existing records from loading fails the build first. Version 2 turned an invoice's single paid status into a list of payments; a version 1 database is upgraded on first open.

`Store::open(&DataDir)` opens the file, and `Store::in_memory()` gives the same database in memory for tests. From a store you get:
- `store.repository()`: a `RedbRepository`, which implements `ProjectRepository`, `InvoiceRepository` and `SettingsRepository`
- `store.ledger()`: an accounting-core `AccountStore` + `TransactionStore`, so `Ledger::new(store.ledger())` works directly

The app opens the file on start, before the terminal is taken over, so an error like "already open" prints normally.

## Roadmap

1. ✅ **TUI layout shell**: tabs, project list and milestone table, footer hints, help popup
2. ✅ **Domain model**: serde, hourly billing and time entries, status transitions, validation
3. ✅ **Storage**: redb database, project repository, ledger storage for `accounting-core`
4. ✅ **Wire storage into the TUI**: load and save, forms to create and edit projects and milestones, log hours
5. ✅ **Invoicing & payments**: tax, export (LUT) and plain invoices, PDFs, ledger posting, payments with TDS, Invoices and Settings tabs
6. ✅ **Tax and reports**: dashboard, expenses, year report (income, 44ADA vs books, GST by quarter, TDS and income by client), quarterly GSTR-1 JSON with exports
7. **Polish**: archive finished projects (hidden from the list, kept for tax records), credit notes, editing expenses, IFF, search and filter, theming
