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

**Steps 1–4 of 7 are done.** compter saves to its database: create, edit and delete projects and milestones, and log hours on hourly work. Every change is saved as you make it. Invoicing and payments arrive in step 5.

## Build and run

```bash
cargo run              # launch compter on your data
cargo run -- --demo    # try it on sample projects kept in memory; nothing is saved
cargo test             # unit, view and storage tests
cargo clippy --all-targets -- -D warnings
```

Only one compter can have the database open at a time; a second one exits with a clear message.

Needs Rust 1.88+ (the `pdf` feature of `accounting-core`) and a terminal with Unicode support.

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
| milestones | `u` | undo the last time entry (asks first) |
| milestones | `i` | invoice *(step 5)* |

**Forms**: type to edit, `Backspace` deletes, `Ctrl-u` clears the field. `Tab`/`↓` and `Shift-Tab`/`↑` move between fields, and `Space`/`←→` change a choice. `Enter` saves and `Esc` cancels. If something is wrong, the form stays open with the reason shown in red and your input kept.

**What you can type**

| Field | Accepts |
|-------|---------|
| amounts | `50000`, `1,50,000.50`, `₹1,50,000` |
| dates | `2026-10-31`; blank means today |
| time | `1h30m`, `2h`, `45m`, `1:30`, or a plain number of **hours** (`1.5` = 1h 30m). The field shows what it will log as you type (`= 1h 30m`). One entry can be at most 24 hours, so `90` is refused rather than logged as 90 hours. |
| GSTIN | 15 characters, any case; blank for an unregistered client |

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
   │   (redb)   │      │ (step 5+)  │     └───────────┘
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
├── demo.rs              sample projects for --demo and tests
├── services/            use-cases: load → domain change → save
│   ├── project_service.rs  ProjectService, ProjectDraft, MilestoneDraft
│   └── error.rs         ServiceError
├── storage/             redb persistence
│   ├── store.rs         Store: open/in-memory, schema check, hands out repositories
│   ├── repository.rs    ProjectRepository trait
│   ├── projects.rs      RedbProjectRepository
│   ├── ledger.rs        RedbLedgerStorage (accounting-core AccountStore + TransactionStore)
│   ├── tables.rs        every table definition
│   ├── codec.rs         JSON encode/decode of values
│   ├── migrations.rs    upgrade steps between schema versions
│   ├── data_dir.rs      where compter.redb lives
│   └── error.rs         StorageError
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
    ├── widgets.rs       panel, modal, money, duration, status_span
    ├── selection.rs     wrap-around Selection and j/k Nav
    ├── form/            reusable modal Form (text and choice fields), Confirm, input parsers
    ├── header.rs  footer.rs  popup.rs
    └── views/
        ├── projects/    ProjectsView: state and keys, rendering, forms
        └── placeholder.rs  Dashboard, Invoices, Reports until their steps
```

Unit tests sit in a `tests/` folder beside the code they cover (`src/domain/tests/`, `src/storage/tests/`, `src/ui/tests/`, and `src/tests/` for `app.rs`), one `<file>_tests.rs` per source file. Integration tests that reopen a real database file are in the top-level `tests/` directory.

Each tab implements `View`. A new screen is a new `View` registered in `main.rs`; the core loop doesn't change. See `CLAUDE.md` for the SOLID, DRY and memory rules the code follows.

## Storage

Everything lives in one [redb](https://www.redb.org) database file, `compter.redb`. redb is an embedded, ACID key-value store. The file goes in the first of these that is set:

1. `$COMPTER_HOME`
2. `$XDG_DATA_HOME/compter`
3. `$HOME/.local/share/compter`

| Table | Key | Value |
|-------|-----|-------|
| `meta` | `"schema_version"` | `1` |
| `projects` | project UUID | full project with milestones and time entries |
| `project_summaries` | project UUID | id, name, client, status, total, for list views |
| `accounts` | account id | accounting-core `Account` |
| `transactions` | transaction id | accounting-core `Transaction` |

- **Atomic**: a project and its summary are written in one transaction, so the list can never disagree with the records. A crash leaves the last committed state.
- **Low memory**: nothing is cached. Lists read only the small summary rows; a full project is read when you open it. The ledger store answers each query from the database.
- **Validated on load**: values are stored as JSON bytes and decoded through the same checks as the domain constructors. A hand-edited bad record fails with its table and key instead of loading.
- **Single process**: redb locks the file, so a second `compter` is refused with a clear error instead of corrupting data.
- **Versioned, with upgrades**: the `meta` table records the schema version. Opening an older database runs the upgrade steps in `storage/migrations.rs` within one transaction. A database from a *newer* compter is refused rather than guessed at. Saved-format fixtures (`storage/tests/fixtures/`) are loaded on every `cargo test`, so a change that would stop existing records from loading fails the build first.

`Store::open(&DataDir)` opens the file, and `Store::in_memory()` gives the same database in memory for tests. From a store you get:
- `store.projects()`: a `ProjectRepository` (list, load, save, delete)
- `store.ledger()`: an accounting-core `AccountStore` + `TransactionStore`, so `Ledger::new(store.ledger())` works directly

The app opens the file on start, before the terminal is taken over, so an error like "already open" prints normally.

## Roadmap

1. ✅ **TUI layout shell**: tabs, project list and milestone table, footer hints, help popup
2. ✅ **Domain model**: serde, hourly billing and time entries, status transitions, validation
3. ✅ **Storage**: redb database, project repository, ledger storage for `accounting-core`
4. ✅ **Wire storage into the TUI**: load and save, forms to create and edit projects and milestones, log hours
5. **Invoicing** (+ invoice and settings tables): milestone → `GstInvoice` (SAC code, CGST+SGST or IGST), validation, preview, PDF export, ledger posting, record payment
6. **Tax and reports**: dashboard, income statement per FY, receivables, GSTR-1 JSON export, per-client income summary for ITR (incl. 44ADA view)
7. **Polish**: archive finished projects (hidden from the list, kept for tax records), search and filter, settings screen, error toasts, theming, integration tests
