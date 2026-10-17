//! Narrowing a list to the rows that match what was typed after `/`

use super::form::{Field, Form};
use super::view::KeyHint;

const QUERY: usize = 0;

/// Words a list is narrowed to; a row matches when every word appears in one of its fields,
/// ignoring case. Empty shows everything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    query: String,
    words: Vec<String>,
}

impl Filter {
    pub const HINT: KeyHint = KeyHint::new("/", "search");
    pub const CLEAR_HINT: KeyHint = KeyHint::new("Esc", "clear search");

    pub fn new(query: &str) -> Self {
        let query = query.trim().to_owned();
        let words = query.split_whitespace().map(str::to_lowercase).collect();
        Self { query, words }
    }

    /// What was typed, for showing in a title
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn matches(&self, fields: &[&str]) -> bool {
        self.words.iter().all(|word| {
            fields
                .iter()
                .any(|field| field.to_lowercase().contains(word.as_str()))
        })
    }

    /// Positions in `items` of the rows that match, `fields` giving each row's searchable text
    pub fn positions<T, const N: usize>(
        &self,
        items: &[T],
        fields: impl Fn(&T) -> [&str; N],
    ) -> Vec<usize> {
        items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.matches(&fields(item)))
            .map(|(index, _)| index)
            .collect()
    }

    /// The search box, holding the current words; blank clears the search
    pub fn form(&self, hint: &'static str) -> Form {
        Form::new(
            "Search",
            vec![Field::text("Contains", hint).with_value(self.query.clone())],
        )
    }

    /// The filter typed into [`Self::form`]
    pub fn from_form(form: &Form) -> Self {
        Self::new(form.value(QUERY))
    }
}
