use std::borrow::Cow;

use crate::ui::key::Key;
use ratatui::text::Span;

use super::list_picker::{ListItem, ListPicker};
use crate::ui::theme::Theme;

/// Longest text a field accepts; plenty for names, notes and amounts
const MAX_INPUT: usize = 120;
/// Choices with more options than this show only the selected one, to fit the line
const INLINE_CHOICES: usize = 3;

/// Describes how a text field's current input will be read, e.g. `= 1h 30m`
pub type Preview = fn(&str) -> String;

/// Works a field's value out from other fields' text (a choice gives its option's label),
/// in the order they are named, e.g. a state from a GSTIN; `None` while it can't, and the field
/// is typed in as usual
pub type Derive = fn(&[&str]) -> Option<String>;

#[derive(Debug, Clone)]
enum Kind {
    Text {
        value: String,
        hint: Cow<'static, str>,
        preview: Option<Preview>,
    },
    Choice {
        options: &'static [&'static str],
        selected: usize,
    },
    /// Several lines of text, like an address; never empty, `line` is the one being typed in
    Lines {
        values: Vec<String>,
        line: usize,
        hint: &'static str,
        max: usize,
    },
}

/// Rows a text field can be filled in from (Ctrl-l): what each row puts in the field, and the
/// row shown for it
#[derive(Debug, Clone)]
struct Choices {
    title: &'static str,
    values: Vec<String>,
    items: Vec<ListItem>,
}

/// A text field worked out from the fields `above` rows up while `derive` gives a value
#[derive(Debug, Clone)]
struct Derived {
    above: &'static [usize],
    derive: Derive,
    hint: &'static str,
    /// What was typed before the field was worked out, put back once it no longer is; `Some`
    /// exactly while the field is derived
    typed: Option<String>,
}

/// One row of a [`super::Form`]
#[derive(Debug, Clone)]
pub struct Field {
    label: &'static str,
    kind: Kind,
    choices: Option<Choices>,
    derived: Option<Derived>,
}

impl Field {
    /// Free text; `hint` is shown while the field is empty
    pub fn text(label: &'static str, hint: impl Into<Cow<'static, str>>) -> Self {
        Self::of(
            label,
            Kind::Text {
                value: String::new(),
                hint: hint.into(),
                preview: None,
            },
        )
    }

    pub fn choice(label: &'static str, options: &'static [&'static str]) -> Self {
        Self::of(
            label,
            Kind::Choice {
                options,
                selected: 0,
            },
        )
    }

    /// Up to `max` lines of text, a row each; ↓ on the last one starts another. Its value is
    /// read with [`super::Form::lines`], and set from text with a line break between lines.
    pub fn lines(label: &'static str, hint: &'static str, max: usize) -> Self {
        Self::of(
            label,
            Kind::Lines {
                values: vec![String::new()],
                line: 0,
                hint,
                max: max.max(1),
            },
        )
    }

    fn of(label: &'static str, kind: Kind) -> Self {
        Self {
            label,
            kind,
            choices: None,
            derived: None,
        }
    }

    /// Let a text field be filled in from a list (Ctrl-l), as well as typed: each entry is the
    /// value it puts in the field and the row shown for it
    #[must_use]
    pub fn with_list(mut self, title: &'static str, entries: Vec<(String, ListItem)>) -> Self {
        let (values, items) = entries.into_iter().unzip();
        self.choices = Some(Choices {
            title,
            values,
            items,
        });
        self
    }

    /// Work a text field out from the fields these many rows up, in this order, whenever
    /// `derive` gives a value for it: the field then shows that value with `hint`, and takes
    /// no input
    #[must_use]
    pub fn derived_from(
        mut self,
        above: &'static [usize],
        derive: Derive,
        hint: &'static str,
    ) -> Self {
        self.derived = Some(Derived {
            above,
            derive,
            hint,
            typed: None,
        });
        self
    }

    /// Prefill a text field
    #[must_use]
    pub fn with_value(mut self, text: impl Into<String>) -> Self {
        self.set_value(text);
        self
    }

    /// Replace a text field's input (a line break between the lines of a lines field); a
    /// choice is left as it is. A derived field keeps it for when it is no longer derived.
    pub(super) fn set_value(&mut self, text: impl Into<String>) {
        let text = text.into();
        if let Some(Derived {
            typed: Some(typed), ..
        }) = &mut self.derived
        {
            *typed = text;
            return;
        }
        match &mut self.kind {
            Kind::Text { value, .. } => *value = text,
            Kind::Lines { values, line, .. } => {
                *values = text.split('\n').map(str::to_owned).collect();
                *line = 0;
            }
            Kind::Choice { .. } => {}
        }
    }

    /// The positions of the fields this one is worked out from, given this one's; empty when
    /// it isn't derived
    pub(super) fn derived_sources(&self, index: usize) -> Vec<usize> {
        self.derived.as_ref().map_or_else(Vec::new, |derived| {
            derived
                .above
                .iter()
                .filter_map(|&above| index.checked_sub(above))
                .collect()
        })
    }

    /// Work the field out again from its sources' text: show the derived value while there
    /// is one, and what was typed before once there isn't
    pub(super) fn rederive(&mut self, sources: &[&str]) {
        let Some(derived) = &mut self.derived else {
            return;
        };
        let Kind::Text { value, .. } = &mut self.kind else {
            return;
        };
        match ((derived.derive)(sources), derived.typed.take()) {
            (Some(worked_out), typed) => {
                let typed = typed.unwrap_or_else(|| std::mem::take(value));
                *value = worked_out;
                derived.typed = Some(typed);
            }
            (None, Some(typed)) => *value = typed,
            (None, None) => {}
        }
    }

    /// Worked out from another field, so it takes no input
    pub fn is_derived(&self) -> bool {
        self.derived.as_ref().is_some_and(|d| d.typed.is_some())
    }

    /// The list to fill the field in from, unless it has none or is derived
    pub(super) fn picker(&self) -> Option<ListPicker> {
        let choices = self.choices.as_ref().filter(|_| self.can_pick())?;
        Some(ListPicker::new(choices.title, choices.items.clone()))
    }

    /// Has a list to fill it in from, and isn't locked by being derived
    pub(super) fn can_pick(&self) -> bool {
        self.choices.is_some() && !self.is_derived()
    }

    /// Fill the field in with the list's row at `index`
    pub(super) fn pick(&mut self, index: usize) {
        let value = self
            .choices
            .as_ref()
            .and_then(|choices| choices.values.get(index))
            .cloned();
        if let Some(value) = value {
            self.set_value(value);
        }
    }

    /// The lines typed into a lines field, trimmed, blank ones left out; empty for others
    pub fn typed_lines(&self) -> Vec<String> {
        match &self.kind {
            Kind::Lines { values, .. } => values
                .iter()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
            Kind::Text { .. } | Kind::Choice { .. } => Vec::new(),
        }
    }

    /// Focus arrives: on the first line of a lines field from above, the last from below
    pub(super) fn enter(&mut self, from_above: bool) {
        if let Kind::Lines { values, line, .. } = &mut self.kind {
            *line = if from_above { 0 } else { values.len() - 1 };
        }
    }

    /// Focus leaves: a lines field drops its blank lines, keeping one to type in
    pub(super) fn leave(&mut self) {
        if let Kind::Lines { values, line, .. } = &mut self.kind {
            values.retain(|value| !value.trim().is_empty());
            if values.is_empty() {
                values.push(String::new());
            }
            *line = 0;
        }
    }

    /// Rows the field takes up in the form
    pub(super) fn height(&self) -> usize {
        match &self.kind {
            Kind::Lines { values, .. } => values.len(),
            Kind::Text { .. } | Kind::Choice { .. } => 1,
        }
    }

    /// Show how the input will be interpreted while it is being typed
    #[must_use]
    pub fn with_preview(mut self, describe: Preview) -> Self {
        if let Kind::Text { preview, .. } = &mut self.kind {
            *preview = Some(describe);
        }
        self
    }

    /// Preselect an option of a choice field
    #[must_use]
    pub fn with_choice(mut self, index: usize) -> Self {
        self.set_choice(index);
        self
    }

    /// Select an option of a choice field; a text field is left as it is
    pub(super) fn set_choice(&mut self, index: usize) {
        if let Kind::Choice { options, selected } = &mut self.kind {
            *selected = index.min(options.len().saturating_sub(1));
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    /// Typed text, or the selected option's label; empty for a lines field (see
    /// [`Field::typed_lines`])
    pub fn value(&self) -> &str {
        match &self.kind {
            Kind::Text { value, .. } => value,
            Kind::Choice { options, selected } => options.get(*selected).copied().unwrap_or(""),
            Kind::Lines { .. } => "",
        }
    }

    /// Index of the selected option (0 for text fields)
    pub fn selected(&self) -> usize {
        match self.kind {
            Kind::Choice { selected, .. } => selected,
            Kind::Text { .. } | Kind::Lines { .. } => 0,
        }
    }

    /// Apply an editing key; `false` when the key means nothing to this field. For a lines
    /// field ↑↓ move between its lines, and are `false` past the first or last, so focus moves.
    pub(super) fn input(&mut self, key: Key) -> bool {
        if self.is_derived() {
            return false;
        }
        match &mut self.kind {
            Kind::Text { value, .. } => edit(value, key),
            Kind::Lines {
                values, line, max, ..
            } => {
                let count = values.len();
                match key {
                    Key::Up if *line > 0 => *line -= 1,
                    Key::Down if *line + 1 < count => *line += 1,
                    // A new line only after one with something on it
                    Key::Down
                        if count < *max && values.last().is_some_and(|v| !v.trim().is_empty()) =>
                    {
                        values.push(String::new());
                        *line = count;
                    }
                    Key::Backspace
                        if *line > 0 && values.get(*line).is_some_and(String::is_empty) =>
                    {
                        values.remove(*line);
                        *line -= 1;
                    }
                    _ => return values.get_mut(*line).is_some_and(|value| edit(value, key)),
                }
                true
            }
            Kind::Choice { options, selected } => {
                let count = options.len().max(1);
                match key {
                    Key::Char(' ') | Key::Right => *selected = (*selected + 1) % count,
                    Key::Left => *selected = (*selected + count - 1) % count,
                    _ => return false,
                }
                true
            }
        }
    }

    /// The field's rows as spans, [`Field::height`] of them
    pub(super) fn rows(&self, focused: bool, theme: &Theme) -> Vec<Vec<Span<'_>>> {
        match &self.kind {
            Kind::Text {
                value,
                hint,
                preview,
            } => {
                if let Some(Derived { hint, .. }) =
                    self.derived.as_ref().filter(|_| self.is_derived())
                {
                    let mut spans = vec![Span::styled(value.as_str(), theme.dim())];
                    if let Some(describe) = preview {
                        spans.push(Span::styled(format!("  {}", describe(value)), theme.dim()));
                    }
                    spans.push(Span::styled(format!(" · {hint}"), theme.dim()));
                    return vec![spans];
                }
                let mut spans = vec![Span::raw(value.as_str())];
                if focused {
                    spans.push(Span::styled("█", theme.key()));
                }
                if value.is_empty() {
                    spans.push(Span::styled(format!("  {hint}"), theme.dim()));
                } else if let Some(describe) = preview {
                    spans.push(Span::styled(format!("  {}", describe(value)), theme.dim()));
                }
                vec![spans]
            }
            Kind::Lines {
                values,
                line,
                hint,
                max,
            } => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let current = focused && index == *line;
                    let mut spans = vec![Span::raw(value.as_str())];
                    if current {
                        spans.push(Span::styled("█", theme.key()));
                    }
                    let last = index + 1 == values.len();
                    if values.len() == 1 && value.is_empty() {
                        spans.push(Span::styled(format!("  {hint}"), theme.dim()));
                    } else if current && last && !value.is_empty() && values.len() < *max {
                        spans.push(Span::styled("  ↓ another line", theme.dim()));
                    }
                    spans
                })
                .collect(),
            Kind::Choice { .. } => vec![self.choice_spans(theme)],
        }
    }

    fn choice_spans(&self, theme: &Theme) -> Vec<Span<'_>> {
        match &self.kind {
            Kind::Text { .. } | Kind::Lines { .. } => Vec::new(),
            Kind::Choice { options, selected } if options.len() > INLINE_CHOICES => {
                let label = options.get(*selected).copied().unwrap_or("");
                vec![
                    Span::styled(format!(" ‹ {label} › "), theme.selected),
                    Span::styled(format!("  {}/{}", selected + 1, options.len()), theme.dim()),
                ]
            }
            Kind::Choice { options, selected } => options
                .iter()
                .enumerate()
                .flat_map(|(index, option)| {
                    let style = if index == *selected {
                        theme.selected
                    } else {
                        theme.dim()
                    };
                    [Span::styled(format!(" {option} "), style), Span::raw(" ")]
                })
                .collect(),
        }
    }
}

/// Apply an editing key to one line of text; `false` when the key means nothing to it
fn edit(value: &mut String, key: Key) -> bool {
    match key {
        Key::Char(c) if !c.is_control() && value.chars().count() < MAX_INPUT => {
            value.push(c);
            true
        }
        Key::Backspace => value.pop().is_some(),
        Key::Ctrl('u') => {
            value.clear();
            true
        }
        _ => false,
    }
}
