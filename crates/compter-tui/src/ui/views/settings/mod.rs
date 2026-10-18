//! Settings tab: the business profile invoices are issued under, and this device's preferences

use std::path::Path;

use crate::ui::key::Key;
use accounting_core::HsnMaster;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::ui::form::{Field, FolderPicker, Form, parse};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::widgets::panel;
use compter_core::domain::{
    Appearance, BusinessProfile, Currency, DEFAULT_INVOICE_PREFIX, DEFAULT_SAC, FiscalYear,
    InvoiceNumber, LogoFormat, ProfileDraft,
};
use compter_core::services::{InvoiceService, PreferencesService};
use compter_core::storage::Repository;
use compter_core::storage::sync::{SyncDir, SyncSource};

const HINTS: &[KeyHint] = &[
    KeyHint::new("e", "edit business details"),
    KeyHint::new("l", "set logo"),
    KeyHint::new("t", "next theme"),
    KeyHint::new("s", "sync folder"),
    KeyHint::new("x", "stop syncing"),
];

const NAME: usize = 0;
const ADDRESS: [usize; 3] = [1, 2, 3];
const GSTIN: usize = 4;
const SAC: usize = 5;
const PREFIX: usize = 6;
const LUT: usize = 7;
const TERMS: [usize; 2] = [8, 9];
const CURRENCY: usize = 10;

const LOGO_FILE: usize = 0;

enum Modal {
    Profile(Form),
    Logo(Form),
    Sync(FolderPicker),
}

/// What the Settings tab shows about the current logo; the bytes stay in storage
#[derive(Debug, Clone, Copy)]
struct LogoInfo {
    format: LogoFormat,
    bytes: usize,
}

/// Shows the business profile; `e` edits it, `l` sets the logo, `t` changes the theme, `s` picks
/// the sync folder and `x` turns syncing off
pub struct SettingsView<R> {
    service: InvoiceService<R>,
    preferences: PreferencesService<R>,
    profile: Option<BusinessProfile>,
    appearance: Appearance,
    logo: Option<LogoInfo>,
    sync: Option<(SyncDir, SyncSource)>,
    modal: Option<Modal>,
}

impl<R: Repository> SettingsView<R> {
    pub fn new(
        service: InvoiceService<R>,
        preferences: PreferencesService<R>,
    ) -> Result<Self, compter_core::services::ServiceError> {
        let profile = service.profile()?;
        let logo = logo_info(&service)?;
        let appearance = preferences.appearance()?;
        let sync = preferences.sync_folder()?;
        Ok(Self {
            service,
            preferences,
            profile,
            appearance,
            logo,
            sync,
            modal: None,
        })
    }

    fn reload(&mut self) -> Result<(), compter_core::services::ServiceError> {
        self.profile = self.service.profile()?;
        self.logo = logo_info(&self.service)?;
        self.appearance = self.preferences.appearance()?;
        self.sync = self.preferences.sync_folder()?;
        Ok(())
    }

    fn submit(&mut self, form: &Form) -> Result<Notice, String> {
        let lines = |fields: &[usize]| fields.iter().map(|&i| form.value(i).to_owned()).collect();
        let draft = ProfileDraft {
            name: form.value(NAME).to_owned(),
            address: lines(&ADDRESS),
            gstin: parse::optional(form.value(GSTIN)),
            sac_code: form.value(SAC).to_owned(),
            invoice_prefix: form.value(PREFIX).to_owned(),
            lut_arn: parse::optional(form.value(LUT)),
            terms: lines(&TERMS),
            currency: Currency::from_index(form.choice(CURRENCY)),
            // The logo is set on its own (`l`); editing the details keeps it
            logo: self.profile.as_ref().and_then(BusinessProfile::logo),
        };
        let profile = self
            .service
            .save_profile(draft)
            .map_err(|e| e.to_string())?;
        self.profile = Some(profile);
        Ok(Notice::success("Business details saved"))
    }

    fn submit_logo(&mut self, form: &Form) -> Result<Notice, String> {
        let path = parse::path(form.value(LOGO_FILE));
        let profile = self
            .service
            .set_logo(path.as_deref())
            .map_err(|e| e.to_string())?;
        self.profile = Some(profile);
        self.logo = logo_info(&self.service).map_err(|e| e.to_string())?;
        Ok(Notice::success(if path.is_some() {
            "Logo saved: it is printed on new invoices"
        } else {
            "Logo removed from new invoices"
        }))
    }

    fn submit_sync(&mut self, path: Option<&Path>) -> Result<Notice, String> {
        self.preferences
            .set_sync_folder(path)
            .map_err(|e| e.to_string())?;
        self.sync = self.preferences.sync_folder().map_err(|e| e.to_string())?;
        Ok(Notice::success(
            match (
                path.is_some(),
                self.sync.as_ref().map(|(_, source)| *source),
            ) {
                (_, Some(SyncSource::Env)) => {
                    "Saved, but $COMPTER_SYNC_DIR overrides it while it is set"
                }
                (true, _) => "Sync folder saved: compter syncs through it from the next start",
                (false, _) => "Sync turned off from the next start",
            },
        ))
    }
}

fn logo_info<R: Repository>(
    service: &InvoiceService<R>,
) -> Result<Option<LogoInfo>, compter_core::services::ServiceError> {
    Ok(service.current_logo()?.map(|logo| LogoInfo {
        format: logo.format(),
        bytes: logo.bytes().len(),
    }))
}

fn logo_form() -> Form {
    Form::new(
        "Logo",
        vec![Field::text(
            "Logo file",
            "path to a PNG or JPEG; blank removes it",
        )],
    )
}

fn profile_form(profile: Option<&BusinessProfile>) -> Form {
    let draft = profile
        .map(BusinessProfile::to_draft)
        .unwrap_or_else(|| ProfileDraft {
            sac_code: DEFAULT_SAC.to_owned(),
            invoice_prefix: DEFAULT_INVOICE_PREFIX.to_owned(),
            ..ProfileDraft::default()
        });
    let line = |lines: &[String], i: usize| lines.get(i).cloned().unwrap_or_default();
    Form::new(
        "Business details",
        vec![
            Field::text("Name", "your name or business name").with_value(draft.name.clone()),
            Field::text("Address", "street").with_value(line(&draft.address, 0)),
            Field::text("", "area").with_value(line(&draft.address, 1)),
            Field::text("", "city, PIN").with_value(line(&draft.address, 2)),
            Field::text("GSTIN", "blank if not registered: invoices without GST")
                .with_value(draft.gstin.clone().unwrap_or_default()),
            Field::text("SAC code", "998314 IT design & development")
                .with_value(draft.sac_code.clone())
                .with_preview(describe_sac),
            Field::text("Prefix", "up to 5 letters, e.g. INV")
                .with_value(draft.invoice_prefix.clone()),
            Field::text("LUT ARN", "needed to invoice clients abroad")
                .with_value(draft.lut_arn.clone().unwrap_or_default()),
            Field::text("Terms", "e.g. Bank: HDFC 0001234, IFSC HDFC0000001")
                .with_value(line(&draft.terms, 0)),
            Field::text("", "e.g. Payment due in 15 days").with_value(line(&draft.terms, 1)),
            Field::choice("Currency", Currency::LABELS).with_choice(draft.currency.index()),
        ],
    )
}

/// What a typed SAC code covers, and its GST rate
fn describe_sac(input: &str) -> String {
    HsnMaster::global().lookup(input.trim()).map_or_else(
        || "not in the GST rate list".to_owned(),
        |entry| format!("{} ({}%)", entry.description, entry.gst_rate),
    )
}

impl<R: Repository> View for SettingsView<R> {
    fn title(&self) -> &'static str {
        "Settings"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = panel("Business profile", false, theme);
        let mut lines = match &self.profile {
            None => vec![
                Line::raw("Not set up yet."),
                Line::default(),
                Line::styled(
                    "Press e to add your business details. They are needed before you can invoice.",
                    theme.dim(),
                ),
            ],
            Some(p) => profile_lines(p, self.logo, theme),
        };
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled(format!("{:<16}", "Theme"), theme.dim()),
            Span::raw(self.appearance.label()),
            Span::styled("   t switches: dark, light, monochrome", theme.dim()),
        ]));
        lines.push(sync_line(self.sync.as_ref(), theme));
        let body = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block);
        frame.render_widget(body, area);
        match &self.modal {
            Some(Modal::Profile(form) | Modal::Logo(form)) => form.render(frame, theme),
            Some(Modal::Sync(picker)) => picker.render(frame, theme),
            None => {}
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        match self.modal.take() {
            Some(Modal::Profile(form)) => {
                let (form, action) = form.step(key, |form| self.submit(form));
                self.modal = form.map(Modal::Profile);
                return action;
            }
            Some(Modal::Logo(form)) => {
                let (form, action) = form.step(key, |form| self.submit_logo(form));
                self.modal = form.map(Modal::Logo);
                return action;
            }
            Some(Modal::Sync(picker)) => {
                let (picker, action) = picker.step(key, |dir| self.submit_sync(Some(dir)));
                self.modal = picker.map(Modal::Sync);
                return action;
            }
            None => {}
        }
        match key {
            Key::Char('e') => {
                self.modal = Some(Modal::Profile(profile_form(self.profile.as_ref())));
                Action::Handled
            }
            Key::Char('l') if self.profile.is_none() => {
                Action::Notify(Notice::error("Add your business details (e) before a logo"))
            }
            Key::Char('l') => {
                self.modal = Some(Modal::Logo(logo_form()));
                Action::Handled
            }
            Key::Char('t') => match self.preferences.next_appearance() {
                Ok(appearance) => {
                    self.appearance = appearance;
                    Action::Theme(appearance)
                }
                Err(error) => Action::Notify(Notice::error(error.to_string())),
            },
            Key::Char('s') => {
                let saved = self
                    .sync
                    .as_ref()
                    .filter(|(_, source)| *source == SyncSource::Settings)
                    .map(|(dir, _)| dir.root());
                self.modal = Some(Modal::Sync(FolderPicker::open(
                    "Choose a folder Google Drive, OneDrive, Dropbox or iCloud syncs",
                    saved,
                )));
                Action::Handled
            }
            Key::Char('x') if self.sync.is_none() => {
                Action::Notify(Notice::info("Sync is already off"))
            }
            Key::Char('x') => match self.submit_sync(None) {
                Ok(notice) => Action::Notify(notice),
                Err(error) => Action::Notify(Notice::error(error)),
            },
            _ => Action::Ignored,
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        match self.modal {
            Some(Modal::Profile(_) | Modal::Logo(_)) => Form::HINTS,
            Some(Modal::Sync(_)) => FolderPicker::HINTS,
            None => HINTS,
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload()
            .err()
            .map(|error| Notice::error(error.to_string()))
    }
}

fn sync_line<'a>(sync: Option<&'a (SyncDir, SyncSource)>, theme: &Theme) -> Line<'a> {
    let label = Span::styled(format!("{:<16}", "Sync folder"), theme.dim());
    let Some((dir, source)) = sync else {
        return Line::from(vec![
            label,
            Span::raw("off"),
            Span::styled("   s picks one to share data between devices", theme.dim()),
        ]);
    };
    let note = match source {
        SyncSource::Env => "   set by $COMPTER_SYNC_DIR, which overrides s",
        SyncSource::Settings => "   s changes it, x stops syncing; from the next start",
    };
    Line::from(vec![
        label,
        Span::raw(dir.root().display().to_string()),
        Span::styled(note, theme.dim()),
    ])
}

fn profile_lines<'a>(
    p: &'a BusinessProfile,
    logo: Option<LogoInfo>,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let row = |label: &'static str, value: String| {
        Line::from(vec![
            Span::styled(format!("{label:<16}"), theme.dim()),
            Span::raw(value),
        ])
    };
    let sac = p.sac_entry().map_or_else(
        || p.sac_code().to_owned(),
        |e| format!("{} {} ({}% GST)", e.code, e.description, e.gst_rate),
    );
    let gstin = p.gstin().map_or_else(
        || "not registered: invoices are issued without GST".to_owned(),
        |g| g.as_str().to_owned(),
    );
    let example = InvoiceNumber::new(
        p.invoice_prefix(),
        FiscalYear::containing(chrono::Local::now().date_naive()),
        1,
    );
    vec![
        row("Name", p.name().to_owned()),
        row("Address", p.address().join(", ")),
        row("GSTIN", gstin),
        row("SAC", sac),
        row("Invoice numbers", format!("{example}, …")),
        row(
            "LUT ARN",
            p.lut_arn()
                .unwrap_or("none: needed to invoice clients abroad")
                .to_owned(),
        ),
        row("Terms", p.terms().join(" · ")),
        row(
            "Currency",
            format!(
                "{} {}: default for new projects; clients in India are billed in INR",
                p.currency().code(),
                p.currency().symbol()
            ),
        ),
        row(
            "Logo",
            logo.map_or_else(
                || "none: press l to print one on new invoices".to_owned(),
                |logo| {
                    format!(
                        "{}, {} KB: printed on new invoices",
                        logo.format.label(),
                        logo.bytes.div_ceil(1024)
                    )
                },
            ),
        ),
    ]
}
