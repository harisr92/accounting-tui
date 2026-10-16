//! Settings tab: the business profile invoices are issued under

use accounting_core::HsnMaster;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use termion::event::Key;

use crate::domain::{
    BusinessProfile, Currency, DEFAULT_INVOICE_PREFIX, DEFAULT_SAC, FiscalYear, InvoiceNumber,
    ProfileDraft,
};
use crate::services::InvoiceService;
use crate::storage::Repository;
use crate::ui::form::{Field, Form, parse};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::widgets::panel;

const HINTS: &[KeyHint] = &[KeyHint::new("e", "edit business details")];

const NAME: usize = 0;
const ADDRESS: [usize; 3] = [1, 2, 3];
const GSTIN: usize = 4;
const SAC: usize = 5;
const PREFIX: usize = 6;
const LUT: usize = 7;
const TERMS: [usize; 2] = [8, 9];
const CURRENCY: usize = 10;

/// Shows the business profile; `e` edits it
pub struct SettingsView<R> {
    service: InvoiceService<R>,
    profile: Option<BusinessProfile>,
    form: Option<Form>,
}

impl<R: Repository> SettingsView<R> {
    pub fn new(service: InvoiceService<R>) -> Result<Self, crate::services::ServiceError> {
        let profile = service.profile()?;
        Ok(Self {
            service,
            profile,
            form: None,
        })
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
        };
        let profile = self
            .service
            .save_profile(draft)
            .map_err(|e| e.to_string())?;
        self.profile = Some(profile);
        Ok(Notice::success("Business details saved"))
    }
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
            Some(p) => profile_lines(p, theme),
        };
        let body = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block);
        frame.render_widget(body, area);
        if let Some(form) = &self.form {
            form.render(frame, theme);
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        if let Some(form) = self.form.take() {
            let (form, action) = form.step(key, |form| self.submit(form));
            self.form = form;
            return action;
        }
        match key {
            Key::Char('e') => {
                self.form = Some(profile_form(self.profile.as_ref()));
                Action::Handled
            }
            _ => Action::Ignored,
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        if self.form.is_some() {
            Form::HINTS
        } else {
            HINTS
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        match self.service.profile() {
            Ok(profile) => {
                self.profile = profile;
                None
            }
            Err(error) => Some(Notice::error(error.to_string())),
        }
    }
}

fn profile_lines<'a>(p: &'a BusinessProfile, theme: &Theme) -> Vec<Line<'a>> {
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
    ]
}
