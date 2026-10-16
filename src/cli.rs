//! Non-interactive commands run from the command line instead of the TUI

use std::path::Path;
use std::process::Command;

use chrono::DateTime;
use color_eyre::Result;
use color_eyre::eyre::{WrapErr, eyre};

use crate::services::ProjectService;
use crate::services::merge_import::{self, WorkSession};
use crate::storage::Store;

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
