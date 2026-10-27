//! Non-interactive commands run from the command line instead of the TUI

use std::path::Path;
use std::process::Command;

use chrono::DateTime;
use color_eyre::Result;
use color_eyre::eyre::{WrapErr, eyre};

use compter_core::services::ProjectService;
use compter_core::services::merge_import::{self, WorkSession};
use compter_core::storage::sync::{self, SyncBackend};
use compter_core::storage::{DataDir, Store};

use crate::drive::{self, DriveFolder};
use crate::viewer;

/// Add a milestone per merge into the main branch of `repo` to the hourly project `project`
pub fn import_merges(store: &Store, repo: &Path, project: &str, dry_run: bool) -> Result<()> {
    let log = git(
        repo,
        &[
            "log",
            "--merges",
            "--first-parent",
            "--reverse",
            "--format=%h%x09%cI%x09%s",
        ],
    )?;
    let root = git(repo, &["log", "--max-parents=0", "--format=%cI", "-1"])?;
    let start = DateTime::parse_from_rfc3339(root.trim())
        .wrap_err_with(|| format!("unexpected root commit date {root:?}"))?;
    let merges = merge_import::parse_log(&log);
    let sessions = merge_import::sessions(start, &merges);

    let mut service = ProjectService::new(store.repository());
    let id = service.find_by_name(project)?;
    if dry_run {
        for session in &sessions {
            println!(
                "{}  {:>5}  {:<45}  {}",
                session.date,
                hours(session.minutes),
                session.name,
                session.hash()
            );
        }
        println!(
            "{} merges, {} in total (dry run: nothing saved)",
            sessions.len(),
            hours(total_minutes(&sessions))
        );
        return Ok(());
    }

    let (project, added) = service.import_sessions(id, &sessions)?;
    println!(
        "added {added} of {} merges to {:?}; project total {}",
        sessions.len(),
        project.name(),
        project.currency().format(project.total_amount())
    );
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .wrap_err("could not run git")?;
    if !output.status.success() {
        return Err(eyre!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).wrap_err("git printed non-UTF-8 output")
}

fn total_minutes(sessions: &[WorkSession]) -> u32 {
    sessions
        .iter()
        .fold(0, |total, s| total.saturating_add(s.minutes))
}

fn hours(minutes: u32) -> String {
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

/// Sign in to Google in the browser and sync this computer through Google Drive from now on.
/// A Drive with no snapshot yet gets this computer's, so the phone finds one.
pub fn drive_sign_in(data: &DataDir, device_name: &str) -> Result<()> {
    let client = drive::Client::from_build()?;
    let account = drive::sign_in(&client, |url| {
        println!("Opening Google sign-in in your browser. If it doesn't open, visit:\n\n{url}\n");
        // The URL is printed for when there is no browser to open
        let _ = viewer::open_url(url);
    })?;
    SyncBackend::save_google_drive(data, Some(&account))?;
    println!("signed in as {account}; compter syncs through Google Drive from the next start");
    let folder = DriveFolder::new(client, &account);
    if let Some(generation) = sync::seed(data, &folder, device_name)
        .wrap_err("could not publish to Google Drive; your data is safe on this computer")?
    {
        println!("published snapshot {generation} to Google Drive");
    }
    Ok(())
}

/// Stop syncing through Google Drive and forget the sign-in on this computer
pub fn drive_sign_out(data: &DataDir) -> Result<()> {
    let Some(SyncBackend::GoogleDrive { account }) = SyncBackend::configured(data, None)? else {
        println!("this computer doesn't sync through Google Drive");
        return Ok(());
    };
    drive::sign_out(&account)?;
    SyncBackend::save_google_drive(data, None)?;
    println!("signed out of {account}; sync is off until you set it up again");
    Ok(())
}
