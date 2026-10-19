//! Rendering only: reads `App` state, never mutates domain data

pub mod filter;
mod footer;
pub mod form;
mod header;
pub mod key;
mod popup;
pub mod selection;
pub mod theme;
pub mod view;
pub mod views;
pub mod widgets;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    header::render(frame, header_area, app);
    app.active_view().render(frame, body_area, app.theme());
    footer::render(frame, footer_area, app);

    if app.show_help() {
        popup::render_help(frame, app);
    }
}

#[cfg(test)]
mod tests;
