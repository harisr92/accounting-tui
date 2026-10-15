use ratatui::text::Span;
use termion::event::Key;

use crate::ui::theme::Theme;

/// Longest text a field accepts; plenty for names, notes and amounts
const MAX_INPUT: usize = 120;

/// Describes how a text field's current input will be read, e.g. `= 1h 30m`
pub type Preview = fn(&str) -> String;

#[derive(Debug, Clone)]
enum Kind {
    Text {
        value: String,
        hint: &'static str,
        preview: Option<Preview>,
    },
    Choice {
        options: &'static [&'static str],
        selected: usize,
    },
}

/// One row of a [`super::Form`]
#[derive(Debug, Clone)]
pub struct Field {
    label: &'static str,
    kind: Kind,
}

impl Field {
    /// Free text; `hint` is shown while the field is empty
    pub fn text(label: &'static str, hint: &'static str) -> Self {
        Self {
            label,
            kind: Kind::Text {
                value: String::new(),
                hint,
                preview: None,
            },
        }
    }

    pub fn choice(label: &'static str, options: &'static [&'static str]) -> Self {
        Self {
            label,
            kind: Kind::Choice {
                options,
                selected: 0,
            },
        }
    }

    /// Prefill a text field
    #[must_use]
    pub fn with_value(mut self, text: impl Into<String>) -> Self {
        if let Kind::Text { value, .. } = &mut self.kind {
            *value = text.into();
        }
        self
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
        if let Kind::Choice { options, selected } = &mut self.kind {
            *selected = index.min(options.len().saturating_sub(1));
        }
        self
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    /// Typed text, or the selected option's label
    pub fn value(&self) -> &str {
        match &self.kind {
            Kind::Text { value, .. } => value,
            Kind::Choice { options, selected } => options.get(*selected).copied().unwrap_or(""),
        }
    }

    /// Index of the selected option (0 for text fields)
    pub fn selected(&self) -> usize {
        match self.kind {
            Kind::Choice { selected, .. } => selected,
            Kind::Text { .. } => 0,
        }
    }

    /// Apply an editing key; `false` when the key means nothing to this field
    pub(super) fn input(&mut self, key: Key) -> bool {
        match &mut self.kind {
            Kind::Text { value, .. } => match key {
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
            },
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

    pub(super) fn spans(&self, focused: bool, theme: &Theme) -> Vec<Span<'_>> {
        match &self.kind {
            Kind::Text {
                value,
                hint,
                preview,
            } => {
                let mut spans = vec![Span::raw(value.as_str())];
                if focused {
                    spans.push(Span::styled("█", theme.key()));
                }
                if value.is_empty() {
                    spans.push(Span::styled(format!("  {hint}"), theme.dim()));
                } else if let Some(describe) = preview {
                    spans.push(Span::styled(format!("  {}", describe(value)), theme.dim()));
                }
                spans
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
