# compter

A terminal app for freelancers who work for several clients, billed by the hour or per feature. Track work as **projects** split into **milestones**, raise GST invoices from milestones, record payments, and get the numbers you need to file income tax and GST returns, all without leaving the terminal.

Built with [ratatui](https://ratatui.rs) (termion backend) on top of [`accounting-core`](https://github.com/harisr92/accounting-blue), which provides double-entry bookkeeping, GST invoices with PDF export, GSTR-1 and financial reports.

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

**Steps 1–2 of 7 are done**: the layout shell and the domain model (billing, time tracking, status transitions, validation, serde). The app still runs on built-in sample data. Nothing is saved yet.

## Build and run

```bash
cargo run              # launch the TUI
cargo test             # unit and render tests
cargo clippy --all-targets -- -D warnings
```

Needs Rust 1.88+ (the `pdf` feature of `accounting-core`) and a terminal with Unicode support.

## Key bindings

| Key | Action |
|-----|--------|
| `1`–`9`, `←`/`→` (`h`/`l`) | switch tab |
| `Tab` | switch focus between the project list and milestones |
| `j`/`k`, `↓`/`↑` | move the selection (wraps around) |
| `n` | new project *(step 4)* |
| `i` | invoice the selected milestone *(step 5)* |
| `?` | help popup (`Esc` closes it) |
| `q`, `Ctrl-c` | quit |

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
| INVOICED | PAID | `record_payment` | |

Once a milestone is invoiced it is locked: no more time entries and it can't be removed. Any other transition returns `DomainError::InvalidTransition`.

**Project status** is derived from the milestones:

| Status | When |
|--------|------|
| ACTIVE | no milestones yet, or any milestone is pending |
| COMPLETED | none pending, at least one invoiced but unpaid |
| PAID | every milestone is paid |

**Serialisation**: projects are serde types (JSON in step 3). Loading runs the same validation as the constructors (names, GSTIN, amounts, rate, billing consistency) and recomputes `total_amount`, `status` and the amounts of pending hourly milestones. Derived values in the file are never trusted. The exception is the amount of an invoiced or paid milestone: it is the billed record, so it is kept as stored (it must be > 0) and does not follow later rate changes.

The financial year is the Indian April–March year (`FY 2026-27`).

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
   │ (step 3)   │      │ (step 5+)  │     └───────────┘
   └────────────┘      └─────┬──────┘
                             ▼
                      accounting-core  (GstInvoice, Ledger, PDF, GSTR-1, reports)
```

```
src/
├── main.rs              binary: panic hook, terminal, view registration, event loop
├── lib.rs               library root; everything below is the `compter_tui` crate
├── app.rs               App state and global key bindings
├── event.rs             termion input and tick threads
├── mock.rs              sample data (removed in step 4)
├── domain/              pure types and rules
│   ├── project.rs       Project, ProjectId, ProjectStatus, mutations, validated serde
│   ├── milestone.rs     Milestone, MilestoneStatus, transitions
│   ├── billing.rs       Billing, TimeEntry, hourly amount calculation
│   ├── error.rs         DomainError
│   └── fiscal_year.rs   Indian April–March year
└── ui/
    ├── mod.rs           draw(): header / body / footer, help overlay
    ├── view.rs          View trait, Action, KeyHint
    ├── theme.rs         every colour and style
    ├── widgets.rs       panel, money, status_span, centered_rect
    ├── selection.rs     wrap-around Selection and j/k Nav
    ├── header.rs  footer.rs  popup.rs
    └── views/           projects (live), placeholder (Dashboard, Invoices, Reports)
```

Unit tests sit in a `tests/` folder beside the code they cover (`src/domain/tests/`, `src/ui/tests/`, and `src/tests/` for `app.rs`), one `<file>_tests.rs` per source file.

Each tab implements `View`. A new screen is a new `View` registered in `main.rs`; the core loop doesn't change. See `CLAUDE.md` for the SOLID, DRY and memory rules the code follows.

## Storage *(step 3)*

Plain files under `~/.local/share/compter/` (override with `$COMPTER_HOME`):

```
projects/index.json        lightweight list: id, name, client, status, total
projects/<id>.json         full project with milestones
invoices/<number>.json     issued GST invoices (+ <number>.pdf)
ledger/accounts.json       chart of accounts
ledger/transactions.json   journal
config.toml                seller name, GSTIN, address, invoice prefix
```

Writes are atomic (temp file, then rename). The ledger files back an `accounting-core` `AccountStore` + `TransactionStore` implementation.

## Roadmap

1. ✅ **TUI layout shell**: tabs, project list and milestone table, footer hints, help popup
2. ✅ **Domain model**: serde, hourly billing and time entries, status transitions, validation
3. **File storage**: JSON repositories, ledger storage for `accounting-core`
4. **Wire storage into the TUI**: load and save, forms to create and edit projects and milestones, log hours
5. **Invoicing**: milestone → `GstInvoice` (SAC code, CGST+SGST or IGST), validation, preview, PDF export, ledger posting, record payment
6. **Tax and reports**: dashboard, income statement per FY, receivables, GSTR-1 JSON export, per-client income summary for ITR (incl. 44ADA view)
7. **Polish**: search and filter, settings screen, error toasts, theming, integration tests
