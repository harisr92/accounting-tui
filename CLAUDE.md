# CLAUDE.md

Engineering rules for `compter-tui`. Follow them for every change. See `README.md` for what the app does and the roadmap.

## Stack

- Rust 2024, `ratatui` 0.30 with the **termion** backend (no crossterm)
- `accounting-core` (git, pinned rev, `pdf` feature) for GST invoices, ledger, reports and returns. Use it; don't reimplement accounting, GST or INR formatting (`accounting_core::utils::formatting::format_inr`)
- `bigdecimal` for money (never `f64`), `chrono::NaiveDate` for dates
- `color_eyre::Result` in `main` and at the edges; typed `thiserror` enums inside `domain` and `storage`

## Module boundaries

The crate is a library (`src/lib.rs`, `compter_tui`) plus a thin binary (`src/main.rs`). Public domain APIs live in the library, so items the UI doesn't use yet aren't dead code, and integration tests can go under `tests/`.

```
domain/    pure types and rules: no I/O, no UI, no termion/ratatui imports
storage/   redb database behind ProjectRepository and the ledger store
services/  use-cases (ProjectService; invoicing, payments, tax later); generic over repository traits
app.rs     application state and global key handling
event.rs   input and tick threads
ui/        rendering and key handling; changes data only by calling services
```

Dependencies point inward: `ui -> app -> services -> domain`, `storage -> domain`. `domain` depends on nothing in this crate.

## Domain invariants

- Domain fields are private. Mutate only through domain methods that validate and then call `refresh()`.
- Derived fields (`total_amount`, project `status`, amounts of *pending* hourly milestones) are recomputed by the domain. Never set them from outside, and never trust them from storage.
- Once a milestone is invoiced its amount is the billed record: it is frozen and never recomputed (e.g. after a rate change).
- Deserialisation goes through a `Raw*` struct + `TryFrom` (`#[serde(try_from = ...)]`) that runs the same `validate()` as the constructors.
- Status changes go through the explicit transition methods (`invoice_milestone`, `record_payment`). No status setters.
- Billing records are never destroyed: invoiced or paid milestones can't be edited or removed, and a project containing any can't be deleted (`Project::ensure_deletable`, checked in the service). New delete paths must respect this.
- Money is `BigDecimal`, rounded with `with_scale_round(2, RoundingMode::HalfUp)`. Durations are whole minutes (`u32`).
- Reuse accounting-core value types (`Gstin`) instead of re-validating strings.

## Storage (redb)

- One database file, `compter.redb`, opened once via `storage::Store`. Code outside `storage/` never touches redb types; it uses `ProjectRepository` or the ledger store.
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
- Tests use `Store::in_memory()`. Tests that reopen the file use `tempfile` and go in the top-level `tests/`.

## SOLID

- **Single responsibility**: one concern per module and per type. Rendering, state changes and persistence never share a function.
- **Views call services, never storage.** A view holds a service (`ProjectService<R>`), not a repository, and never calls domain mutators on its loaded copy. Every change is load → domain method → save inside the service (`ProjectService::update`), so a rejected change is never saved and the view re-reads what was stored.
- **Open/closed**: every tab is a `ui::view::View`. Add a screen by implementing `View` and registering it in `main.rs`. Don't add per-tab `match` arms to `App` or `ui::draw`.
- **Liskov**: any `View` must work through `&dyn View`. A view returns `Action::Ignored` for keys it doesn't own so global bindings still work.
- **Interface segregation**: narrow traits (`ProjectRepository`, `InvoiceRepository`, `SettingsRepository`), not one god storage trait.
- **Dependency inversion**: services and views take data or traits through their constructors. Concrete types (`FileStorage`) are only built in `main.rs`. Tests use in-memory implementations.

## DRY

- Colours and styles come only from `ui::theme::Theme`. No `Color::` literals in views.
- Use the shared helpers in `ui::widgets`: `panel` (bordered block with focus styling), `modal` (cleared, centred, focused box), `money`, `duration`, `status_span`/`Toned`, `centered_rect`.
- List and table navigation goes through `ui::selection::{Selection, Nav}`. Don't hand-roll wrap-around or j/k mapping.
- Key hints are defined once as `&'static [KeyHint]`. The footer and the help popup both render from that one slice. The footer always shows `FOOTER_HINTS` (`? help · q quit`) on the right; view hints fill the left and may be cut off on narrow screens.
- All input goes through `ui::form`: build a `Form` from `Field::text` / `Field::choice`, and use `Confirm` before anything irreversible. Don't write another text-input widget.
- Parse what the user typed only in `ui/form/parse.rs` (money, dates, durations, optional text). Give a field a live `with_preview` when its input could be misread (durations).
- Modals belong to the view that opened them. While one is open the view takes **every** key, so typing never triggers shortcuts. Only `Ctrl-c` is handled by `App` first.
- A failed submit keeps the form open: put the reason in `Form::set_error` and keep the input. Errors outside forms go to the footer as `Notice::error`.
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
src/domain/billing.rs     → src/domain/tests/billing_tests.rs
src/ui/selection.rs       → src/ui/tests/selection_tests.rs
src/ui/mod.rs (draw)      → src/ui/tests/draw_tests.rs
src/app.rs (crate root)   → src/tests/app_tests.rs
```

- The parent file ends with `#[cfg(test)] mod tests;`: `domain/mod.rs`, `ui/mod.rs`, and `lib.rs` for crate-root files like `app.rs`. A new module (`storage/`, `services/`) gets its own `tests/` folder the same way.
- `tests/mod.rs` only declares the files: `mod billing_tests;`. Keep the declarations sorted.
- Import explicitly (`use crate::domain::{Billing, Project};`), never `use super::*`.
- Test through the public API. Reach into a private module (e.g. `crate::domain::billing::amount_for`) only for pure helpers.
- `unwrap`/`expect` are fine in tests. Name tests after the behaviour (`transitions_cannot_be_skipped_or_repeated`), not the function.
- Cross-module integration tests (e.g. FileStorage on a temp dir, step 3) go in the top-level `tests/` directory and use the `compter_tui` library crate.

## Before you call a change done

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

UI changes also need a manual `cargo run` check: tabs, focus, help popup, resizing, and a clean exit with `q`. Add or extend the `TestBackend` render test in `src/ui/tests/draw_tests.rs` for new chrome.

Shell commands are proxied through RTK (`rtk ...`) automatically. See `~/.claude/RTK.md`.
