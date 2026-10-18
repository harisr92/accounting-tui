# CLAUDE.md

Engineering rules for `compter-tui`. Follow them for every change. See `README.md` for what the app does and the roadmap.

## Stack

- Rust 2024, `ratatui` 0.30 with the **crossterm** backend (`ratatui::crossterm`, no separate dependency), so it runs on macOS, Linux and Windows. No termion: it has no Windows support
- Views see only `ui::key::Key`. Crossterm events are converted once, in `Key::from_event` (which drops key releases, reported on Windows)
- `accounting-core` (git, pinned rev, `pdf` feature) for GST invoices, ledger, reports and returns. Use it; don't reimplement accounting, GST or INR formatting (`accounting_core::utils::formatting::format_inr`)
- `bigdecimal` for money (never `f64`), `chrono::NaiveDate` for dates
- `color_eyre::Result` in `main` and at the edges; typed `thiserror` enums inside `domain` and `storage`

## Module boundaries

A Cargo workspace of three crates, plus the Flutter app in `app/`. `crates/compter-core` (`compter_core`) is the product with no UI, so every front end (the terminal app now, a mobile app later) shares it. `crates/compter-tui` (`compter_tui`) is the terminal app: a library plus a thin binary (`src/main.rs`). Public APIs live in the libraries, so items a UI doesn't use yet aren't dead code.

```
compter-core/src/
  domain/    pure types and rules: no I/O, no UI
  storage/   redb database behind the repository traits and the ledger store
  services/  use-cases (ProjectService, InvoiceService, ...); generic over repository traits
  demo.rs    in-memory sample business for --demo and test fixtures
compter-tui/src/
  app.rs     application state and global key handling
  event.rs   input, resize and tick threads
  cli.rs     non-interactive commands (import-merges reads git here)
  viewer.rs  opens a written PDF in the desktop's default app
  ui/        rendering and key handling; changes data only by calling services
compter-mobile/src/
  api/       what the Flutter app calls (flutter_rust_bridge): CompterApp over the services,
             plain rows of strings, ApiError { field, message }
```

`compter-mobile` follows the view rules: it calls services only and hands Dart plain rows (formatted money, `YYYY-MM-DD` dates), never domain types. Typed text goes to Rust unparsed and is parsed by `compter_core::input`, the parsers the terminal's `ui/form/parse.rs` re-exports. No data-carrying enums under `api` (Dart would need freezed). After changing `api`, run `flutter_rust_bridge_codegen generate && cargo fmt --all` in `app/` (see `app/README.md`). It is not a default workspace member, because flutter_rust_bridge compiles C that desktop cross-builds can't.

Dependencies point inward: `ui -> app -> services -> domain`, `storage -> domain`. `domain` depends on nothing in this workspace. `compter-core` never depends on ratatui, crossterm, color-eyre, or anything that assumes a desktop: no spawning processes, and env vars are read only by the `from_env` helpers (`DataDir::from_env`, `SyncDir::from_env`, `sync::device_name_from_env`), which only a desktop binary calls; other front ends inject the values (`DataDir::new`). Services return file paths; showing a file is the front end's job (`viewer::open`).

## Domain invariants

- Domain fields are private. Mutate only through domain methods that validate and then call `refresh()`.
- Derived fields (`total_amount`, project `status`, amounts of *pending* hourly milestones) are recomputed by the domain. Never set them from outside, and never trust them from storage.
- Once a milestone is invoiced its amount is the billed record: it is frozen and never recomputed (e.g. after a rate change).
- Deserialisation goes through a `Raw*` struct + `TryFrom` (`#[serde(try_from = ...)]`) that runs the same `validate()` as the constructors.
- Status changes go through the explicit transition methods (`invoice_milestone`, `record_payment`). No status setters.
- Schema history: v2 replaced an invoice's `status` with `payments` (`storage/migrations.rs::payments_list`). Domain decoding refuses a v1 invoice (`OutdatedInvoiceFormat`) rather than read a paid one as unpaid. v3 added credit notes and `MilestoneStatus::Credited` with no rows to rewrite (`credit_notes` step), so older builds refuse a file they would misread.
- Billing records are never destroyed: invoiced or paid milestones can't be edited or removed, and a project containing any can't be deleted (`Project::ensure_deletable`, checked in the service). New delete paths must respect this.
- Money is `BigDecimal`, rounded with `with_scale_round(2, RoundingMode::HalfUp)`. Durations are whole minutes (`u32`).
- Reuse accounting-core value types (`Gstin`) instead of re-validating strings.

## Storage (redb)

- One database file, `compter.redb`, opened once via `storage::Store`. Code outside `storage/` never touches redb types; it uses the repository traits (via `store.repository()`) or the ledger store.
- One `RedbRepository` implements every repository trait, so a change spanning several records shares one write transaction. Services and views name only the traits they need (`R: ProjectRepository`), or `R: Repository` when they need all three.
- Multi-table writes go through a record struct (`IssueRecord`, `PaymentRecord`) and one trait method that writes everything with the shared helpers in `redb_repository.rs` (`put_project`, `put_invoice`, `put_transaction`, `put_missing_accounts`). Don't chain separate repository calls for one logical change.
- Every `TableDefinition` lives in `storage/tables.rs`. A new table also goes into `Store::init`.
- Users' real records live in this file, so **stored types evolve compatibly**. This covers any type that is serialised into a table: `Project`, `Milestone`, `TimeEntry`, `Billing`, `ProjectSummary`, and later invoices and settings.
  - **Adding a field**: give it `#[serde(default)]` (or `Option<_>`) so older records still load. No version bump.
  - **Renaming, removing or retyping a field, or changing an enum's tags**: bump `SCHEMA_VERSION` and append an upgrade step to `storage/migrations.rs` that rewrites the old rows. Steps run in `Store::init`, inside the same write transaction as the version bump, so a failed upgrade leaves the file untouched.
  - Never use `#[serde(deny_unknown_fields)]` on stored types.
  - The fixtures in `storage/tests/fixtures/*_v1.json` are records in the shipped format. **Never edit them** to make a test pass. If `format_tests` fails, the change is breaking: add a default or a migration. When the format version changes, add new fixtures next to the old ones.
  - A database with a newer version than `SCHEMA_VERSION` is refused (`UnsupportedSchema`); never guess at it.
- Values are encoded and decoded only through `storage/codec.rs` (JSON bytes). Don't switch to postcard or bincode: they can't decode internally tagged enums like `Billing`.
- Decoding goes through the domain's validated `try_from`. Storage never builds domain types any other way.
- A logical write that touches several tables (a project + its summary) happens in **one** write transaction. Return before `commit()` to abort.
- redb's per-operation errors convert into `StorageError` via `?`. Map `StorageError` to accounting-core errors with `LedgerError::from`.
- accounting-core's store traits are async; we implement them with sync redb calls and drive them with `pollster::block_on`. Don't add an async runtime.
- Sync decides "changed since the last sync" from `Store::content_hash` (every record), never the file's bytes: redb rewrites its header on every open. The manifest's SHA-256 stays a file hash, to spot a half-synced snapshot.
- Phones hand the database over by hand (`sync::receive`, `send`, `settle_received`, with the store closed), with no lease. `receive` takes only a snapshot newer than, or the very one, it last synced; anything else is `SyncError::Undelivered`, since a hand-off may never arrive or arrive as a copy.
- Settings that belong to one device (the sync folder) live in `device.json` in the data dir, not in redb: sync runs before the store opens, and the database travels between devices. Read and write them only through `storage::sync` (`SyncDir::saved` / `save`), via `PreferencesService`. `$COMPTER_SYNC_DIR` overrides the saved folder; it is read only in `main.rs` and injected.
- Tests use `Store::in_memory()`. Tests that reopen the file use `tempfile` and go in `crates/compter-core/tests/`.

## Invoicing

- Invoices are billing records: **immutable once issued**. Only `Invoice::record_payment` and `Invoice::apply_credit` change one, by appending to its payments or its credits. Correct an invoice with a credit note; never edit or delete one.
- Numbers come only from `InvoiceRepository::next_sequence` + `InvoiceNumber::new`, and `issue` refuses a stale sequence. Never build a number by hand or reuse one.
- The kind (tax / export / plain) is decided in one place, `services::invoice_builder::build`, from the seller's GSTIN and the client's region. Tax invoices go through accounting-core (`GstInvoice`, `validate_invoice`, `InvoicePrint::from_invoice`, `to_entries`). Don't recompute GST in compter.
- An invoice stores its `InvoicePrint` and `terms`. PDFs are rendered from those (`render_pdf`), never rebuilt from the current project or settings.
- The logo is snapshotted like the terms: each invoice stores the `LogoId` it was issued with, and `render` loads that logo, never the profile's current one. Logos live in the `logos` table as raw image bytes (the one table not written through `codec`), checked by `Logo::from_stored` on load, and rows are never deleted. `set_logo` test-renders a new logo so an image accounting-core can't draw is refused in Settings.
- Render the PDF **before** saving an invoice, so an unprintable document is never issued. Write the file after the commit; a failed write is reported, never rolled back.
- Ledger account ids live only in `services/chart.rs`. Every posting is checked with `DefaultTransactionValidator`, and its accounts are passed along so storage creates any that are missing.
- Payments may be partial: each is `{date, received, tds}`, and their sum never exceeds the total (`Invoice::record_payment`, checked again on load). Invoice status (Issued / PartlyPaid / Paid / Credited) is derived from the payments and credits, never stored. The milestone is marked paid only by the payment or credit note that settles the invoice.
- Credit notes (`domain::CreditNote`) are billing records too, numbered `CN/<yy-yy>/<seq>` from `next_credit_sequence` and written with the invoice they credit in one `CreditRecord`. `services::credit_note_builder::build` is the one place a note is made: accounting-core's `CreditNote` for tax invoices, compter's own print for export and plain ones, always at the invoice's rate. A note credits only what is still due (never a refund), and `credit_note_deadline` (30 November after the invoice's year) is enforced for tax and export invoices. Every note is recorded on its milestone (`Project::credit_milestone`, before GST), so `paid_amount` counts only money received. A note that leaves nothing due settles the milestone: `cancel_milestone` (CREDITED) when nothing was paid, `record_payment` (PAID) otherwise. The note's posting is `cn-<number>`, reversing the invoice's legs onto `4190 Credit Notes`.
- Each payment posts its own ledger transaction, `pay-<number>-<n>`: Dr Bank (received), Dr `1250 TDS Receivable` (tds), Cr Receivable (both). Payments from schema v1 kept their original id `pay-<number>`.

### Currency

- `domain::Currency` (INR, USD, GBP, EUR). The profile's currency is only the default for new projects; a project's currency lives on its `ClientDetails` and is fixed once a milestone is invoiced.
- Only clients abroad are billed in a foreign currency (`ForeignCurrencyInIndia`), so tax invoices are always INR. Format amounts with `Currency::format` / `widgets::money_in`, never by hand.
- A foreign invoice stores its `currency` and `ExchangeRate` (rupees per unit, entered when issuing). `amounts()` are in the invoice currency. GST, the ledger, GSTR-1 and reports use `inr_amounts()` / `to_inr`.
- Foreign payments settle the balance in the invoice currency and record `credited_inr`, the rupees banked. They carry no TDS. Banked rupees minus the receivable at the invoice rate post to `4900 Exchange Gain/Loss`, and reports show it as `exchange_difference`.
- PDFs label amounts with `Currency::pdf_label` (`Rs.` for INR, because the built-in font has no `₹`).

## Reports and returns

- Reports are computed on demand and never stored. `services::reports` is pure (a `YearReportBuilder` fed one invoice at a time). `ReportService` streams invoices with `InvoiceRepository::scan_invoices`, so it never holds them all in memory.
- Year-bound figures use `FiscalYear` / `Quarter`. Don't compare month numbers by hand. Billing counts by invoice date; payments and TDS count by payment date.
- Don't use accounting-core's `generate_income_statement` for a year: it sums balances *as of* the end date, so earlier years leak in. Build year figures from invoice and expense records.
- GSTR-1 is quarterly (QRMP): `services::gstr1` builds each month with accounting-core's `Gstr1Return::build` (invoices and GST credit notes) and merges them. compter adds only what accounting-core lacks: 6A exports, `cdnur`/`EXPWOP` for credit notes on exports, and table 13. The monthly IFF is the same month build, cut to `b2b` and `cdnr`; months filed by IFF lose their B2B sections from the quarter but keep HSN and table 13. When accounting-core gains quarters or exports, delete compter's version.
- Reports subtract a credit note in the year and quarter of its own date (`YearReportBuilder::add_credit_note`).
- Returns are written under `<data dir>/returns/<year>/` with `write_file`. Plain invoices never go into a return.
- Expenses post like invoices: one transaction (`exp-<id>`) with the expense, written together through `ExpenseRepository`. Input credit needs a GSTIN in the profile. Editing an expense replaces its posting, and deleting one removes it, in the same write.
- Tabs that show a year use `ui::views::year::YearPicker` (`[` / `]`). Rows of figures use `widgets::stat_tiles`.
- Table headers come from `widgets::header(titles, right_aligned_columns, theme)`. Amount and duration cells are right-aligned, and their titles must be too, so they line up (checked by `ui/tests/alignment_tests.rs`).

## SOLID

- **Single responsibility**: one concern per module and per type. Rendering, state changes and persistence never share a function.
- **Views call services, never storage.** A view holds services (`ProjectService<R>`, `InvoiceService<R>`), not a repository, and never calls domain mutators on its loaded copy. Every change is load → domain method → save inside the service (`ProjectService::update`), so a rejected change is never saved and the view re-reads what was stored.
- **Open/closed**: every tab is a `ui::view::View`. Add a screen by implementing `View` and registering it in `main.rs`. Don't add per-tab `match` arms to `App` or `ui::draw`.
- **Liskov**: any `View` must work through `&dyn View`. A view returns `Action::Ignored` for keys it doesn't own so global bindings still work.
- **Interface segregation**: narrow traits (`ProjectRepository`, `InvoiceRepository`, `SettingsRepository`), not one god storage trait.
- Settings that aren't the business profile (the theme) go through `PreferencesService`, not `InvoiceService`.
- **Dependency inversion**: services and views take data or traits through their constructors. Concrete types (`FileStorage`) are only built in `main.rs`. Tests use in-memory implementations.

## DRY

- Colours and styles come only from `ui::theme::Theme`. No `Color::` literals in views. Themes are presets chosen by `domain::Appearance` (saved in settings, switched with `Action::Theme`); `NO_COLOR` forces the monochrome one. A new style is a `Theme` field set in every preset.
- Use the shared helpers in `ui::widgets`: `panel` (bordered block with focus styling), `modal` (cleared, centred, focused box), `money`, `duration`, `status_span`/`Toned`, `centered_rect`.
- List and table navigation goes through `ui::selection::{Selection, Nav}`. Don't hand-roll wrap-around or j/k mapping.
- Searchable lists use `ui::filter::Filter` (`/` opens it, `Esc` clears it) and keep a `visible: Vec<usize>` of positions into their rows; the selection moves over `visible`.
- Archived projects (`Project::archive`, finished only) stay in storage and reports; only the Projects list hides them.
- Key hints are defined once as `&'static [KeyHint]`. The footer and the help popup both render from that one slice. The footer always shows `FOOTER_HINTS` (`? help · q quit`) on the right; view hints fill the left and may be cut off on narrow screens.
- All input goes through `ui::form`: build a `Form` from `Field::text` / `Field::choice`, use `Confirm` before anything irreversible, and `FolderPicker` (run with `step`, like a form) to choose a folder. Don't write another text-input widget.
- Parse what the user typed only in `compter_core::input` (money, dates, durations, optional text), re-exported by `ui/form/parse.rs`, which adds only paths. Give a field a live `with_preview` when its input could be misread (durations).
- Modals belong to the view that opened them. While one is open the view takes **every** key, so typing never triggers shortcuts. Only `Ctrl-c` is handled by `App` first.
- Run forms through `Form::step(key, submit)`: it handles cancel, submit, and keeping the form open with the error on a failed submit. Errors outside forms go to the footer as `Notice::error`.
- A form shared by two tabs (payment) lives in `ui/views/payment.rs`; don't copy form builders between views.
- A view that shows data another tab can change re-reads it in `View::on_activate`.
- Status labels live once on the enum (`label()` / `Display`).

## Memory

- Render from borrowed state: `Span::raw(project.name())`, `&'static str` for fixed text. Don't clone domain data in `render`.
- Formatting dynamic values (money, dates) while rendering is fine. Only redraw when something changed: the `App` dirty flag and `take_dirty()`.
- Fieldless `Copy` enums for statuses, `Copy` newtype ids (`ProjectId(Uuid)`), `u32` minutes. Use `Vec::with_capacity` when the size is known. No `Rc<RefCell<_>>` graphs.
- Channels are bounded (`sync_channel`). Producers that can drop work (ticks) use `try_send`.
- Storage: lists read summary tables (`project_summaries`), never full records. Load a full record only when it is opened. Repositories and the ledger store cache nothing in memory.
- Views hold summary rows plus the **one** selected full record (`ProjectsView::selected`), dropped before the next is loaded. After a change, take the record the service returns instead of loading it again.

## Style

- Match the surrounding code. Doc comments on public types say *why* or *what it represents*, not a restatement of the name.
- No `unwrap`/`expect` outside tests. Use `unwrap_or_default`, `?` or a typed error.
- Keep `main.rs` to wiring: hooks, terminal setup, view registration, event loop.

## Tests

Every module keeps its unit tests in a `tests/` folder next to its source files, one `<file>_tests.rs` per source file. Never write inline `#[cfg(test)] mod tests { … }` blocks.

```
compter-core/src/domain/billing.rs  → compter-core/src/domain/tests/billing_tests.rs
compter-tui/src/ui/selection.rs     → compter-tui/src/ui/tests/selection_tests.rs
compter-tui/src/ui/mod.rs (draw)    → compter-tui/src/ui/tests/draw_tests.rs
compter-tui/src/app.rs (crate root) → compter-tui/src/tests/app_tests.rs
```

- The parent file ends with `#[cfg(test)] mod tests;`: `domain/mod.rs`, `ui/mod.rs`, and `lib.rs` for crate-root files like `app.rs`. A new module (`storage/`, `services/`) gets its own `tests/` folder the same way.
- `tests/mod.rs` only declares the files: `mod billing_tests;`. Keep the declarations sorted.
- Import explicitly (`use crate::domain::{Billing, Project};`), never `use super::*`.
- Test through the public API. Reach into a private module (e.g. `crate::domain::billing::amount_for`) only for pure helpers.
- `unwrap`/`expect` are fine in tests. Name tests after the behaviour (`transitions_cannot_be_skipped_or_repeated`), not the function.
- Cross-module integration tests (e.g. a database file on a temp dir) go in `crates/compter-core/tests/` and use the `compter_core` library crate.

## Before you call a change done

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Changes to `crates/compter-mobile` or `app/` also need, in `app/`: `cargo build -p compter-mobile && flutter analyze && flutter test`, and bindings regenerated as above (CI fails on stale ones, `.github/workflows/mobile.yml`).

UI changes also need a manual `cargo run` check: tabs, focus, help popup, resizing, and a clean exit with `q`. Add or extend the `TestBackend` render test in `crates/compter-tui/src/ui/tests/draw_tests.rs` for new chrome.

Shell commands are proxied through RTK (`rtk ...`) automatically. See `~/.claude/RTK.md`.
