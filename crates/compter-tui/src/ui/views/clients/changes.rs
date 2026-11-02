//! Changing the opened client: correcting their details, renaming them, or merging them into
//! another client. Each change that would reach their open projects asks first whether it
//! should.

use crate::ui::form::{Confirm, Field, Form, ListEvent, ListItem, ListPicker};
use crate::ui::key::Key;
use crate::ui::view::{Action, KeyHint, Notice};
use crate::ui::views::client_form;
use compter_core::domain::{ClientDetails, ClientId, ProjectSummary};
use compter_core::services::ServiceError;
use compter_core::storage::{ClientRepository, InvoiceRepository, ProjectRepository};

use super::{ClientsView, NOT_OPEN, Opened};

/// Asking whether the open projects take the change too
pub(super) const PENDING_HINTS: &[KeyHint] = &[
    KeyHint::new("y", "update projects too"),
    KeyHint::new("n", "client only"),
    KeyHint::new("Esc", "back"),
];

/// Never repeated in a question: issued invoices keep what they were printed with
const INVOICES_STAY: &str = "Issued invoices never change.";

/// Choosing the client the opened one is a duplicate of
pub(super) struct Merging {
    pub(super) picker: ListPicker,
    /// The client of each row of `picker`, with their name
    targets: Vec<(ClientId, String)>,
}

/// A change waiting on an answer, with the form or picker it came from, which `Esc` or a
/// refused save goes back to
pub(super) struct Pending {
    pub(super) confirm: Confirm,
    change: Change,
    /// Whether `y` / `n` choose about the open projects; otherwise they confirm or back out
    asks_projects: bool,
}

impl Pending {
    pub(super) fn hints(&self) -> &'static [KeyHint] {
        if self.asks_projects {
            PENDING_HINTS
        } else {
            Confirm::HINTS
        }
    }
}

enum Change {
    Edit {
        details: ClientDetails,
        form: Form,
    },
    Rename {
        name: String,
        form: Form,
    },
    Merge {
        into: ClientId,
        into_name: String,
        merging: Merging,
    },
}

impl Change {
    /// The question about the `projects` named, for the client called `client`
    fn question(&self, client: &str, projects: &[&str]) -> String {
        let (count, names) = (projects.len(), projects.join(", "));
        match self {
            Self::Edit { .. } => format!(
                "Update {count} open project(s) too: {names}? n saves the client only. \
                 {INVOICES_STAY}"
            ),
            Self::Rename { name, .. } => format!(
                "Rename {client} to {name} on {count} open project(s) too: {names}? n renames \
                 the client only; the old name stays as an alias. {INVOICES_STAY}"
            ),
            Self::Merge { into_name, .. } if projects.is_empty() => format!(
                "Merge {client} into {into_name}, keeping {into_name}'s details? {client} \
                 becomes an alias. {INVOICES_STAY}"
            ),
            Self::Merge { into_name, .. } => format!(
                "Merge {client} into {into_name}, keeping {into_name}'s details, and give \
                 {count} open project(s) the merged name and details too: {names}? n merges \
                 the clients only. {INVOICES_STAY}"
            ),
        }
    }
}

impl<R: ClientRepository + ProjectRepository + InvoiceRepository> ClientsView<R> {
    /// The opened client's details in a form; the name stays, so it is the form's subject
    pub(super) fn start_edit(&mut self) -> Action {
        let Some(opened) = &self.opened else {
            return Action::Ignored;
        };
        let fields = client_form::fields(&opened.account.client.details());
        self.editing = Some(Form::new("Edit client", fields));
        Action::Notify(Notice::info(format!(
            "Editing {}; issued invoices keep the details they were printed with",
            opened.account.client.name()
        )))
    }

    pub(super) fn start_rename(&mut self) -> Action {
        let Some(opened) = &self.opened else {
            return Action::Ignored;
        };
        let name = opened.account.client.name();
        let field = Field::text("Name", "what to call them from now on").with_value(name);
        self.renaming = Some(Form::new("Rename client", vec![field]));
        Action::Notify(Notice::info(format!(
            "Renaming {name}; their issued invoices keep it, and it stays on as an alias"
        )))
    }

    /// Pick from the other clients the one the opened client is a duplicate of
    pub(super) fn start_merge(&mut self) -> Action {
        let Some(opened) = &self.opened else {
            return Action::Ignored;
        };
        let client = &opened.account.client;
        let (items, targets): (Vec<_>, Vec<_>) = self
            .clients
            .iter()
            .map(|c| (&c.summary.client, &c.place))
            .filter(|(other, _)| other.id() != client.id())
            .map(|(other, place)| {
                (
                    ListItem::new(other.name(), place.as_str()),
                    (other.id(), other.name().to_owned()),
                )
            })
            .unzip();
        if items.is_empty() {
            return Action::Notify(Notice::info("There is no other client to merge into"));
        }
        self.merging = Some(Merging {
            picker: ListPicker::new("Merge into", items),
            targets,
        });
        Action::Notify(Notice::info(format!(
            "Pick the client {} is a duplicate of; their details are kept",
            client.name()
        )))
    }

    /// Check the corrected details, then ask about the open projects that would change, or
    /// save straight away when there are none
    pub(super) fn submit_edit(&mut self, form: &Form) -> Result<Notice, String> {
        let client = &self.opened.as_ref().ok_or(NOT_OPEN)?.account.client;
        let (id, details) = (client.id(), client_form::read(form, 0, client.name())?);
        let stale = self.service.projects_to_update(id, &details);
        let form = form.clone();
        self.ask_or_save(stale, Change::Edit { details, form })
    }

    pub(super) fn submit_rename(&mut self, form: &Form) -> Result<Notice, String> {
        let id = self.opened.as_ref().ok_or(NOT_OPEN)?.account.client.id();
        let name = form.value(0).trim().to_owned();
        let stale = self.service.projects_to_rename(id, &name);
        let form = form.clone();
        self.ask_or_save(stale, Change::Rename { name, form })
    }

    /// A key for the merge picker: a pick asks before merging, as a merge can't be undone
    pub(super) fn step_merge(&mut self, mut merging: Merging, key: Key) -> Action {
        let picked = match merging.picker.handle_key(key) {
            ListEvent::Picked(row) => merging.targets.get(row).cloned(),
            ListEvent::Cancel => return Action::Handled,
            ListEvent::Edited | ListEvent::Ignored => None,
        };
        let (Some((into, into_name)), Some(opened)) = (picked, &self.opened) else {
            self.merging = Some(merging);
            return Action::Handled;
        };
        let from = opened.account.client.id();
        let stale = match self.service.projects_to_merge(from, into) {
            Ok(stale) => stale,
            Err(error) => {
                self.merging = Some(merging);
                return Action::Notify(Notice::error(error.to_string()));
            }
        };
        let change = Change::Merge {
            into,
            into_name,
            merging,
        };
        let names: Vec<_> = stale.iter().map(|p| p.name()).collect();
        let question = change.question(opened.account.client.name(), &names);
        self.pending = Some(Pending {
            confirm: Confirm::new(question),
            change,
            asks_projects: !stale.is_empty(),
        });
        Action::Handled
    }

    /// Ask about the open projects in `stale` that would change, or save when there are none.
    /// An error keeps the form open with it.
    fn ask_or_save(
        &mut self,
        stale: Result<Vec<ProjectSummary>, ServiceError>,
        change: Change,
    ) -> Result<Notice, String> {
        let stale = stale.map_err(|e| e.to_string())?;
        if stale.is_empty() {
            return self.save(&change, false);
        }
        let client = &self.opened.as_ref().ok_or(NOT_OPEN)?.account.client;
        let names: Vec<_> = stale.iter().map(|p| p.name()).collect();
        self.pending = Some(Pending {
            confirm: Confirm::new(change.question(client.name(), &names)),
            change,
            asks_projects: true,
        });
        Ok(Notice::info(
            "Choose whether their open projects change too",
        ))
    }

    /// `y` or `n` saves (or, for a merge with no projects to ask about, `n` backs out); `Esc`
    /// goes back to the form or picker, as does a refused save, with the error
    pub(super) fn answer_pending(&mut self, pending: Pending, key: Key) -> Action {
        if key == Key::Esc {
            return self.back(pending.change, None);
        }
        let Some(yes) = Confirm::answer(key) else {
            self.pending = Some(pending);
            return Action::Handled;
        };
        if !pending.asks_projects && !yes {
            return self.back(pending.change, None);
        }
        match self.save(&pending.change, pending.asks_projects && yes) {
            Ok(notice) => Action::Notify(notice),
            Err(error) => self.back(pending.change, Some(error)),
        }
    }

    /// Return to where `change` came from, showing `error` if there is one
    fn back(&mut self, change: Change, error: Option<String>) -> Action {
        match change {
            Change::Edit { form, .. } => {
                self.editing = Some(with_error(form, error));
                Action::Handled
            }
            Change::Rename { form, .. } => {
                self.renaming = Some(with_error(form, error));
                Action::Handled
            }
            Change::Merge { merging, .. } => {
                self.merging = Some(merging);
                error.map_or(Action::Handled, |e| Action::Notify(Notice::error(e)))
            }
        }
    }

    fn save(&mut self, change: &Change, with_projects: bool) -> Result<Notice, String> {
        let id = self.opened.as_ref().ok_or(NOT_OPEN)?.account.client.id();
        let (account, done) = match change {
            Change::Edit { details, .. } => (
                self.service.update(id, details.clone(), with_projects),
                "Client saved",
            ),
            Change::Rename { name, .. } => (
                self.service.rename(id, name, with_projects),
                "Client renamed",
            ),
            Change::Merge { into, .. } => (
                self.service.merge(id, *into, with_projects),
                "Clients merged",
            ),
        };
        let account = account.map_err(|e| e.to_string())?;
        match (&mut self.opened, change) {
            // Another client now: their account, cursors from the top
            (opened, Change::Merge { .. }) => *opened = Some(Opened::new(account)),
            (Some(opened), _) => opened.replace(account),
            (None, _) => {}
        }
        self.reload_list().map_err(|e| e.to_string())?;
        Ok(Notice::success(if with_projects {
            format!("{done}, with their open projects")
        } else {
            done.to_owned()
        }))
    }
}

/// `form` showing `error`, if there is one
fn with_error(mut form: Form, error: Option<String>) -> Form {
    if let Some(error) = error {
        form.set_error(error);
    }
    form
}
