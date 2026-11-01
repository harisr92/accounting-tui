---
name: project-review
description: Review changed Rust (compter-core, compter-tui, compter-mobile) and Flutter (app/) code against this project's CLAUDE.md rules — design principles, SOLID, OOP (encapsulation, abstraction, polymorphism, composition), DRY, layering, domain invariants, stored-format compatibility and test layout. Use after every code-changing task, before reporting it done, when asked to review a diff or branch, or after a refactor. Green clippy, tests and flutter analyze do not prove these rules hold, so run this pass anyway.
---

# Project review

Checks the current change against the root `CLAUDE.md` (Stack, Module boundaries, Domain invariants, Storage, Invoicing, Reports, SOLID, DRY, Memory, Style, Tests). CLAUDE.md is the source of truth: re-read the sections the change touches before every review. This skill doesn't restate the rules. It adds the procedure and the checks most often missed.

## 1. Scope the change

```bash
git status --short              # untracked files count too
git diff --stat HEAD
git diff HEAD -- crates app/lib app/test
```

Read every new file in full, and every changed hunk together with its whole function or widget. Review only what changed, plus the code it now calls. Skip generated files (`frb_generated*`, `app/lib/src/rust/`), but check they were regenerated when `crates/compter-mobile/src/api` changed.

## 2. Walk each new or changed type, function and widget

For each one, answer these questions. A "no" is a finding.

### Design principles

- **Dependencies point inward:** `ui -> app -> services -> domain`, `storage -> domain`. Does `domain` import anything from storage, services or the UI? Does `compter-core` use ratatui, crossterm, color-eyre, `std::process`, or read env vars outside the `from_env` helpers?
- **Views call services, never storage:** does a TUI view or a mobile `api` function touch a repository or redb type, or call a domain mutator on its loaded copy? Every change should be load → domain method → save, inside a service.
- **One place for each decision:**
  - invoice kind: `invoice_builder::build`
  - credit notes: `credit_note_builder::build`
  - ledger account ids: `services/chart.rs`
  - milestone order: `MilestoneSort::positions`
  - parsing typed input: `compter_core::input`
  - money formatting: `Currency::format` / `display`

  A second copy of any of these is a finding.
- **Front ends get plain data:** `compter-mobile` hands Dart only rows of strings (formatted money, `YYYY-MM-DD` dates), no domain types, and no data-carrying enums under `api`. Dart never formats money, parses input, sorts milestones or computes tax.
- **Least surprise:** does a name say what the thing represents? Does a function do only what its name says?

### SOLID

**SRP: one reason to change**
- Does one function both render and change state, or change state and persist? Split it.
- Does a view, screen or widget build forms, call services *and* lay out the page in one large function? A TUI view's `render` should only render. A Flutter `build` should only compose widgets.
- Does a function run over about 60 lines, or a Flutter widget's `build` over about 120?

**OCP: extend by adding, not by editing a selector**
- Does a new tab add a `match` arm to `App` or `ui::draw`? Every tab is a `View` registered in `main.rs`.
- Does a new sync storage get named outside `sync/storages.dart`, or get branched on `SyncMethod`?
- Is a `match`/`switch` over a kind, status or currency there to pick an implementation or a style? Prefer a method on the enum (`label()`, `Theme` fields), or a table.

**LSP**
- Does every `View` work through `&dyn View`, returning `Action::Ignored` for keys it doesn't own?
- Does every `SyncFolder` / `CloudStorage` honour its contract? No `todo!()`, no silently different behaviour.

**ISP**
- Does a service or view take `R: Repository` when it needs only `ProjectRepository` (or one other narrow trait)?
- Does a function take a whole struct (`Invoice`, `Project`, `BusinessProfile`) when it reads only two fields?

**DIP**
- Are concrete types (`Store`, `RedbRepository`, `DriveFolder`, `DataDir::from_env`) built anywhere but `main.rs` (or `CompterApp::over` on mobile)?
- Does code read `Local::now()` deep inside logic that should take `today: NaiveDate` as a parameter, which makes it hard to test?
- Do Flutter widgets read `compterProvider` for data, instead of going through a provider in `compter.dart`?

### OOP principles

- **Encapsulation:** domain fields stay private. Is any field made `pub` or given a setter? Is a derived field (`total_amount`, project `status`, invoice status) set from outside, or trusted from storage? Changes go through the validating domain methods that call `refresh()`.
- **Abstraction:** does a caller know about redb, codec JSON or the file layout, instead of using the repository traits or `sync::SyncFolder`?
- **Polymorphism:** is behaviour picked through the trait (`View`, `SyncFolder`, `CloudStorage`, `FormDialog`), rather than by checking concrete types or adding flags?
- **Composition over inheritance:** in Dart, does a new widget extend another app widget to change it? Compose it instead (shared pieces such as `RecordTile`, `FigureGrid`, `ScreenList`, `LargeTitle`, `TitleBar`). Rust: no `Rc<RefCell<_>>` object graphs.
- **Invariants live in the type:** is a status changed without its transition method (`invoice_milestone`, `record_payment`, `apply_credit`)? Is a value type re-validated as a string instead of reusing it (`Gstin`, `InvoiceNumber`, `ExchangeRate`)?

### Domain and billing safety

- Is anything changing an issued invoice or credit note other than `record_payment` / `apply_credit`? Is an invoiced or paid milestone edited or removed? Is a new delete path missing `Project::ensure_deletable`?
- Is an invoice number built by hand instead of `next_sequence` + `InvoiceNumber::new`?
- Is GST recomputed in compter instead of going through accounting-core? Is money held in `f64`, or rounded other than `with_scale_round(2, HalfUp)`?
- Is the PDF rendered *before* saving, with the file written after the commit?

### Stored-format compatibility (any type serialised into redb)

- Does a new field have `#[serde(default)]` or `Option`? Does a rename, removal or retype come with a `SCHEMA_VERSION` bump and a step in `storage/migrations.rs`?
- Any `deny_unknown_fields`? Any edit to `storage/tests/fixtures/*_v1.json`? Both are findings, never fixes.
- Does decoding go through a `Raw*` struct + `TryFrom` that runs `validate()`? Is a multi-table write done in one transaction, through a record struct?
- Is a new `TableDefinition` declared in `storage/tables.rs` and created in `Store::init`?

### DRY / reuse

Search before accepting a new helper:
- **TUI:** `ui::widgets::{panel, modal, money, money_in, duration, stat_tiles, header, centered_rect}`, `ui::selection`, `ui::filter`, `ui::form` (`Form`, `Field`, `Confirm`, `FolderPicker`), `views::year::YearPicker`, `views/payment.rs`.
- **Flutter:** `FormDialog.show`, `showActionSheet`, `confirm`, `notify`, `RecordTile`, `FigureGrid`, `SectionHeader`, `EmptyState`, `CollapsingFab`, `requireWrite`, `reloadEverything`.

Also check for:
- `Color::` literals in TUI views (styles come only from `Theme`), or hard-coded colours in Flutter instead of `Theme.of(context).colorScheme`.
- Key hints defined in more than one place, or status labels spelled out in more than one place.
- A form builder copied between two views.

### Memory and performance

- Do lists read summary tables (`project_summaries`, `list_invoices`) instead of full records? Do whole-database passes stream (`scan_invoices`, `scan_projects`)?
- Is domain data cloned in `render`? Is a record loaded again when the service already returned it? Is a channel unbounded?

### Safety and style

- Any `unwrap()` or `expect()` outside tests? Any `println!`? Any `todo!`?
- Does a doc comment say *why* or *what it represents*, rather than repeat the name?
- On mobile, does every Dart path that creates or changes a record call `requireWrite` first (or go through `FormDialog.show` without `writes: false`)?

### Tests

- Does every new pure function, service path and domain rule have a test named after its behaviour?
- Rust tests live in `<dir>/tests/<file>_tests.rs`, declared (sorted) in `tests/mod.rs`, with explicit imports and no `use super::*`, and are never inline. Tests that reopen a database file go in `crates/compter-core/tests/`.
- Does new TUI chrome extend `ui/tests/draw_tests.rs`? Do new amount columns keep `alignment_tests` passing?
- Is a new Flutter screen or flow covered by a widget test in `app/test/` against the real core (`launchDemo`)? Does a new `compter-mobile` api function have a test in `crates/compter-mobile/tests/`?

## 3. Quick greps on the diff

These are pointers, not verdicts. Read each hit in context.

```bash
git diff HEAD -U0 -- crates | grep -nE '^\+.*(\.unwrap\(\)|\.expect\(|println!|eprintln!|todo!|unimplemented!|f64|f32|Color::|deny_unknown_fields|std::env::var|Command::new)' | grep -v '/tests/'
git diff HEAD -U0 -- crates/compter-core/src/domain | grep -nE '^\+\s*pub [a-z_]+:'
git diff HEAD -U0 -- crates | grep -nE '^\+.*use (redb|ratatui|crossterm)' 
git diff HEAD -U0 -- app/lib | grep -nE "^\+.*(Color\(0x|Colors\.|print\(|toStringAsFixed|NumberFormat|DateFormat|\.sort\(|double\.parse|int\.parse)" | grep -v 'lib/src/rust/'
git diff HEAD --name-only -- crates | xargs grep -ln 'mod tests\s*{' 2>/dev/null   # should print nothing
git diff HEAD --name-only -- crates/compter-core/src/storage/tests/fixtures         # should print nothing
```

## 4. Fix, then verify

- Fix every finding before reporting the work done. If a fix would be a large redesign or a judgement call, list it and ask instead.
- Then run the checks from CLAUDE.md:
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```
  When `crates/compter-mobile` or `app/` changed, also run `cargo clippy -p compter-mobile --all-targets -- -D warnings` and `cargo test -p compter-mobile` (it isn't a default workspace member). In `app/`, run `flutter_rust_bridge_codegen generate && cargo fmt --all` if `api` changed, then `flutter analyze && flutter test`.
- UI changes still need the manual `cargo run` check that CLAUDE.md describes. If you couldn't run it, say so.

## 5. Report

List each finding as `file:line`, the rule broken (with its CLAUDE.md section or principle), and what you changed. Leave out the rules that passed. Say plainly if any check couldn't run, and why.
