use std::path::PathBuf;

use accounting_core::{Gstin, HsnMaster};
use chrono::{Datelike, NaiveDate};

use super::ServiceError;
use super::gstr1::{MonthlyIff, QuarterlyGstr1, monthly_iff, quarterly_gstr1};
use super::reports::{YearReport, YearReportBuilder};
use crate::domain::{CreditNote, FiscalYear, Invoice, InvoiceKind, Quarter};
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
        self.repo
            .scan_credit_notes(&mut |note| builder.add_credit_note(&note))?;
        builder.add_expenses(&self.repo.list_expenses(year)?);
        Ok(builder.finish())
    }

    /// The quarter's GSTR-1, keeping only that quarter's tax and export invoices and credit
    /// notes in memory. The B2B part of the months in `furnished` was filed in an IFF.
    pub fn gstr1(
        &self,
        quarter: Quarter,
        furnished: &[(i32, u32)],
    ) -> Result<QuarterlyGstr1, ServiceError> {
        let filer = self.filer()?;
        let in_quarter = |kind, date| kind != InvoiceKind::Plain && quarter.contains(date);
        let (invoices, notes) = self.documents(
            |i| in_quarter(i.kind(), i.date()),
            |n| in_quarter(n.kind(), n.date()),
        )?;
        Ok(quarterly_gstr1(
            &filer,
            quarter,
            &invoices,
            &notes,
            furnished,
            HsnMaster::global(),
        )?)
    }

    /// The IFF of one of a quarter's first two months: its B2B invoices and credit notes
    pub fn iff(&self, month: (i32, u32)) -> Result<MonthlyIff, ServiceError> {
        let filer = self.filer()?;
        let in_month = |kind, date: NaiveDate| {
            kind == InvoiceKind::Tax && (date.year(), date.month()) == month
        };
        let (invoices, notes) = self.documents(
            |i| in_month(i.kind(), i.date()),
            |n| in_month(n.kind(), n.date()),
        )?;
        Ok(monthly_iff(
            &filer,
            month,
            &invoices,
            &notes,
            HsnMaster::global(),
        )?)
    }

    /// Write the month's IFF JSON under `returns/<year>/` and return where it went
    pub fn export_iff(&self, month: (i32, u32)) -> Result<(PathBuf, MonthlyIff), ServiceError> {
        let dir = self
            .data_dir
            .as_ref()
            .ok_or(ServiceError::NoReturnsFolder)?;
        let iff = self.iff(month)?;
        let bytes =
            serde_json::to_vec_pretty(&iff.json).map_err(accounting_core::Gstr1Error::from)?;
        let (year, number) = month;
        let fiscal = NaiveDate::from_ymd_opt(year, number, 1)
            .map(FiscalYear::containing)
            .ok_or(accounting_core::Gstr1Error::InvalidPeriod {
                year,
                month: number,
            })?;
        let path = dir.return_path(&fiscal.label(), &format!("IFF-{year}-{number:02}"));
        write_file(&path, &bytes)?;
        Ok((path, iff))
    }

    /// The seller's GSTIN; returns need one
    fn filer(&self) -> Result<Gstin, ServiceError> {
        let profile = self
            .repo
            .load_profile()?
            .ok_or(ServiceError::NotRegistered)?;
        profile.gstin().cloned().ok_or(ServiceError::NotRegistered)
    }

    /// The invoices and credit notes a return needs, read one at a time
    fn documents(
        &self,
        keep_invoice: impl Fn(&Invoice) -> bool,
        keep_note: impl Fn(&CreditNote) -> bool,
    ) -> Result<(Vec<Invoice>, Vec<CreditNote>), ServiceError> {
        let mut invoices = Vec::new();
        self.repo.scan_invoices(&mut |invoice| {
            if keep_invoice(&invoice) {
                invoices.push(invoice);
            }
        })?;
        let mut notes = Vec::new();
        self.repo.scan_credit_notes(&mut |note| {
            if keep_note(&note) {
                notes.push(note);
            }
        })?;
        Ok((invoices, notes))
    }

    /// Write the quarter's GSTR-1 JSON under `returns/<year>/` and return where it went
    pub fn export_gstr1(
        &self,
        quarter: Quarter,
        furnished: &[(i32, u32)],
    ) -> Result<(PathBuf, QuarterlyGstr1), ServiceError> {
        let dir = self
            .data_dir
            .as_ref()
            .ok_or(ServiceError::NoReturnsFolder)?;
        let gstr1 = self.gstr1(quarter, furnished)?;
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
