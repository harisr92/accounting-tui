# CLAUDE.md

Engineering rules for `compter-tui`. Follow them for every change. See `README.md` for what the app does and the roadmap.

## Stack

- Rust 2024, `ratatui` 0.30 with the **termion** backend (no crossterm)
- `accounting-core` (git, pinned rev, `pdf` feature) for GST invoices, ledger, reports and returns. Use it; don't reimplement accounting, GST or INR formatting (`accounting_core::utils::formatting::format_inr`)
- `bigdecimal` for money (never `f64`), `chrono::NaiveDate` for dates
- `color_eyre::Result` in `main` and at the edges; typed `thiserror` enums inside `domain` and `storage`

## Module boundaries

```
domain/    pure types and rules: no I/O, no UI, no termion/ratatui imports
storage/   file I/O behind repository traits (step 3)
services/  use-cases: invoicing, payments, tax; depend on repository traits (step 5+)
app.rs     application state and global key handling
event.rs   input and tick threads
ui/        rendering only; reads state, never mutates domain data or touches storage
```

Dependencies point inward: `ui -> app -> services -> domain`, `storage -> domain`. `domain` depends on nothing in this crate.

## SOLID

- **Single responsibility**: one concern per module and per type. Rendering, state changes and persistence never share a function.
- **Open/closed**: every tab is a `ui::view::View`. Add a screen by implementing `View` and registering it in `main.rs`. Don't add per-tab `match` arms to `App` or `ui::draw`.
- **Liskov**: any `View` must work through `&dyn View`. A view returns `Action::Ignored` for keys it doesn't own so global bindings still work.
- **Interface segregation**: narrow traits (`ProjectRepository`, `InvoiceRepository`, `SettingsRepository`), not one god storage trait.
- **Dependency inversion**: services and views take data or traits through their constructors. Concrete types (`FileStorage`) are only built in `main.rs`. Tests use in-memory implementations.

## DRY

- Colours and styles come only from `ui::theme::Theme`. No `Color::` literals in views.
- Use the shared helpers in `ui::widgets`: `panel` (bordered block with focus styling), `money`, `status_span`/`Toned`, `centered_rect`.
- List and table navigation goes through `ui::selection::{Selection, Nav}`. Don't hand-roll wrap-around or j/k mapping.
- Key hints are defined once as `&'static [KeyHint]`. The footer and the help popup both render from that one slice.
- Status labels live once on the enum (`label()` / `Display`).

## Memory

- Render from borrowed state: `Span::raw(project.name.as_str())`, `&'static str` for fixed text. Don't clone domain data in `render`.
- Formatting dynamic values (money, dates) while rendering is fine. Only redraw when something changed: the `App` dirty flag and `take_dirty()`.
- Fieldless `Copy` enums for statuses. Use `Vec::with_capacity` when the size is known. No `Rc<RefCell<_>>` graphs.
- Channels are bounded (`sync_channel`). Producers that can drop work (ticks) use `try_send`.
- Storage (step 3+): load a small project index for lists and load full records only on selection. Write files with `BufWriter` and atomic tmp-then-rename.

## Style

- Match the surrounding code. Doc comments on public types say *why* or *what it represents*, not a restatement of the name.
- No `unwrap`/`expect` outside tests. Use `unwrap_or_default`, `?` or a typed error.
- Keep `main.rs` to wiring: hooks, terminal setup, view registration, event loop.

## Before you call a change done

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

UI changes also need a manual `cargo run` check: tabs, focus, help popup, resizing, and a clean exit with `q`. Add or extend the `TestBackend` render test in `main.rs` for new chrome.

Shell commands are proxied through RTK (`rtk ...`) automatically. See `~/.claude/RTK.md`.
