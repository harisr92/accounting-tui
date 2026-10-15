use std::path::PathBuf;

use accounting_core::HsnMaster;

use super::ServiceError;
use super::gstr1::{QuarterlyGstr1, quarterly_gstr1};
use super::reports::{YearReport, YearReportBuilder};
use crate::domain::{FiscalYear, InvoiceKind, Quarter};
use crate::storage::{
    DataDir, ExpenseRepository, InvoiceRepository, SettingsRepository, write_file,
};

/// Year reports and GST returns, computed on demand from the stored invoices and expenses
pub struct ReportService<R> {
    repo: R,
    data_dir: Option<DataDir>,
}

impl<R: InvoiceRepository + ExpenseRepository + SettingsRepository> ReportService<R> {
    pub fn new(repo: R, data_dir: Option<DataDir>) -> Self {
        Self { repo, data_dir }
    }

    pub fn year(&self, year: FiscalYear) -> Result<YearReport, ServiceError> {
        let mut builder = YearReportBuilder::new(year);
        self.repo
            .scan_invoices(&mut |invoice| builder.add_invoice(&invoice))?;
        builder.add_expenses(&self.repo.list_expenses(year)?);
        Ok(builder.finish())
    }

    /// The quarter's GSTR-1, keeping only that quarter's tax and export invoices in memory
    pub fn gstr1(&self, quarter: Quarter) -> Result<QuarterlyGstr1, ServiceError> {
        let profile = self
            .repo
            .load_profile()?
            .ok_or(ServiceError::NotRegistered)?;
        let filer = profile.gstin().ok_or(ServiceError::NotRegistered)?;
        let mut invoices = Vec::new();
        self.repo.scan_invoices(&mut |invoice| {
            if invoice.kind() != InvoiceKind::Plain && quarter.contains(invoice.date()) {
                invoices.push(invoice);
            }
        })?;
        Ok(quarterly_gstr1(
            filer,
            quarter,
            &invoices,
            HsnMaster::global(),
        )?)
    }

    /// Write the quarter's GSTR-1 JSON under `returns/<year>/` and return where it went
    pub fn export_gstr1(
        &self,
        quarter: Quarter,
    ) -> Result<(PathBuf, QuarterlyGstr1), ServiceError> {
        let dir = self
            .data_dir
            .as_ref()
            .ok_or(ServiceError::NoReturnsFolder)?;
        let gstr1 = self.gstr1(quarter)?;
        let bytes =
            serde_json::to_vec_pretty(&gstr1.json).map_err(accounting_core::Gstr1Error::from)?;
        let path = dir.return_path(
            &quarter.year().label(),
            &format!("GSTR1-Q{}-{}", quarter.number(), quarter.year().label()),
        );
        write_file(&path, &bytes)?;
        Ok((path, gstr1))
    }
}
