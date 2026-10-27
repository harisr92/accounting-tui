//! Turning what the user typed into domain values. Text shared with every front end is parsed by
//! `compter_core::input`; paths, which only the terminal takes, are parsed here.

use std::path::PathBuf;

pub use compter_core::input::{
    ParseResult, date_or, discount, duration_minutes, fiscal_year_or, money, optional,
};

/// `$HOME`, or `%USERPROFILE%` on Windows: what a leading `~` stands for
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// A file path, or `None` when blank. Strips the quotes a terminal adds when a file is dragged
/// in, and expands a leading `~` to the home directory.
pub fn path(input: &str) -> Option<PathBuf> {
    let input = input.trim();
    let input = ['"', '\'']
        .iter()
        .find_map(|&q| input.strip_prefix(q).and_then(|s| s.strip_suffix(q)))
        .unwrap_or(input)
        .trim();
    if input.is_empty() {
        return None;
    }
    let expanded = match input.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            home_dir().map(|home| home.join(rest.trim_start_matches(['/', '\\'])))
        }
        _ => None,
    };
    Some(expanded.unwrap_or_else(|| PathBuf::from(input)))
}
