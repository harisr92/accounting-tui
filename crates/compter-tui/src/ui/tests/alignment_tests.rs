//! Money columns are right-aligned, so their headers must end where their values end

use crate::ui::key::Key;

use crate::ui::tests::support::{go_to_year, invoicing_app, reporting_app, screen};
use compter_core::demo;

const W: u16 = 130;
const H: u16 = 36;

/// The first line below the panel titled `panel` (title line skipped) that contains `text`
fn line_with<'a>(screen: &'a str, panel: &str, text: &str) -> &'a str {
    screen
        .lines()
        .skip_while(|line| !line.contains(panel))
        .skip(1)
        .find(|line| line.contains(text))
        .unwrap_or_else(|| panic!("{text:?} not found below {panel:?}\n{screen}"))
}

/// Character columns just past the end of every occurrence of `text` in `line`
fn right_edges(line: &str, text: &str) -> Vec<usize> {
    line.match_indices(text)
        .map(|(start, _)| line[..start].chars().count() + text.chars().count())
        .collect()
}

/// `title` ends in the same column as one of the `value`s on the first row that has it
fn assert_aligned(screen: &str, panel: &str, title: &str, value: &str) {
    let title_edge = right_edges(line_with(screen, panel, title), title)[0];
    let value_edges = right_edges(line_with(screen, panel, value), value);
    assert!(
        value_edges.contains(&title_edge),
        "{title:?} is not over {value:?} in {panel:?}\n{screen}"
    );
}

#[test]
fn invoice_payments_line_up() {
    let store = demo::store().unwrap();
    let mut app = invoicing_app(&store);
    app.handle_key(Key::Char('2'));
    // Newest first: NDC/26-27/005, then the half-paid NDC/26-27/004
    app.handle_key(Key::Char('j'));
    let screen = screen(&app, W, H);
    assert_aligned(&screen, "Payments for", "Received", "₹35,400.00");
    assert_aligned(&screen, "Payments for", "Settled", "₹35,400.00");
    assert_aligned(&screen, "Invoices (", "Total", "₹70,800.00");
}

#[test]
fn report_clients_and_gst_line_up() {
    let store = demo::store().unwrap();
    let mut app = reporting_app(&store);
    app.handle_key(Key::Char('3'));
    go_to_year(&mut app, 2026);
    let screen = screen(&app, W, H);
    assert_aligned(&screen, "Clients", "TDS", "₹5,000.00");
    assert_aligned(&screen, "Clients", "Invoiced", "₹1,10,000.00");
    assert_aligned(&screen, "GST", "Input credit", "₹28,980.00");
}
