//! Settings tab: the business profile invoices are issued under

use crate::ui::key::Key;
use accounting_core::HsnMaster;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::domain::{
    BusinessProfile, Currency, DEFAULT_INVOICE_PREFIX, DEFAULT_SAC, FiscalYear, InvoiceNumber,
    LogoFormat, ProfileDraft,
};
use crate::services::InvoiceService;
use crate::storage::Repository;
use crate::ui::form::{Field, Form, parse};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::widgets::panel;

const HINTS: &[KeyHint] = &[
    KeyHint::new("e", "edit business details"),
    KeyHint::new("l", "set logo"),
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
}

/// What the Settings tab shows about the current logo; the bytes stay in storage
#[derive(Debug, Clone, Copy)]
struct LogoInfo {
    format: LogoFormat,
    bytes: usize,
}

/// Shows the business profile; `e` edits it, `l` sets the logo
pub struct SettingsView<R> {
    service: InvoiceService<R>,
    profile: Option<BusinessProfile>,
    logo: Option<LogoInfo>,
    modal: Option<Modal>,
}

impl<R: Repository> SettingsView<R> {
    pub fn new(service: InvoiceService<R>) -> Result<Self, crate::services::ServiceError> {
        let profile = service.profile()?;
        let logo = logo_info(&service)?;
        Ok(Self {
            service,
            profile,
            logo,
            modal: None,
        })
    }

    fn reload(&mut self) -> Result<(), crate::services::ServiceError> {
        self.profile = self.service.profile()?;
        self.logo = logo_info(&self.service)?;
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
}

fn logo_info<R: Repository>(
    service: &InvoiceService<R>,
) -> Result<Option<LogoInfo>, crate::services::ServiceError> {
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
        let lines = match &self.profile {
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
        let body = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block);
        frame.render_widget(body, area);
        if let Some(Modal::Profile(form) | Modal::Logo(form)) = &self.modal {
            form.render(frame, theme);
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
            _ => Action::Ignored,
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        if self.modal.is_some() {
            Form::HINTS
        } else {
            HINTS
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload()
            .err()
            .map(|error| Notice::error(error.to_string()))
    }
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
