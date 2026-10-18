//! What the Flutter app can call. Values cross the bridge as plain structs of strings: money is
//! formatted and input is parsed here, by `compter_core`, so Dart never does either.

// Public so flutter_rust_bridge writes one Dart file per module (`api/app.dart`, ...)
pub mod app;
pub mod error;
pub mod expenses;
pub mod handoff;
pub mod invoicing;
pub mod profile;
pub mod reports;
pub mod rows;

pub use app::CompterApp;
pub use error::ApiError;
pub use expenses::{ExpenseInput, ExpenseList, ExpenseRow};
pub use handoff::{ReceivedRow, SentRow, SyncStatusRow};
pub use invoicing::{
    CreditInput, CreditPreviewRow, CreditedRow, HistoryRow, InvoiceDetail, InvoiceList,
    InvoicePreviewRow, InvoiceRow, IssuedRow, PaymentInput, TimeInput,
};
pub use profile::{ProfileInput, ProfileRow};
pub use reports::{ClientRow, ExportRow, FigureRow, QuarterGstRow, ReportRow, Tone};
pub use rows::{MilestoneInput, MilestoneRow, ProjectDetail, ProjectInput, ProjectRow};
