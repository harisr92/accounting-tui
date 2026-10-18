//! Turn a repository's merge commits into hourly milestones: one per merge, timed from the one
//! before. Pure: reading git happens at the edge (compter-tui's `cli`).

use chrono::{DateTime, FixedOffset, NaiveDate};

const SECONDS_PER_MINUTE: i64 = 60;
/// A gap spanning several days counts as at most one working day
pub const MAX_MULTI_DAY_MINUTES: u32 = 8 * 60;

/// One merge commit, as listed by `git log --format=%h%x09%cI%x09%s`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge {
    pub hash: String,
    pub at: DateTime<FixedOffset>,
    pub subject: String,
}

/// Work billed for one merge: becomes a milestone with a single time entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkSession {
    pub name: String,
    pub date: NaiveDate,
    pub minutes: u32,
    /// `<hash> <subject>`; the hash marks the merge as imported
    pub note: String,
}

impl WorkSession {
    /// The merge hash this session was built from
    pub fn hash(&self) -> &str {
        self.note.split_whitespace().next().unwrap_or_default()
    }
}

/// Parse tab-separated `hash, ISO date, subject` lines; malformed lines are skipped
pub fn parse_log(text: &str) -> Vec<Merge> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let hash = fields.next()?.trim();
            let at = DateTime::parse_from_rfc3339(fields.next()?.trim()).ok()?;
            let subject = fields.next()?.trim();
            (!hash.is_empty()).then(|| Merge {
                hash: hash.to_owned(),
                at,
                subject: subject.to_owned(),
            })
        })
        .collect()
}

/// `feat/x` from `Merge branch 'feat/x' into 'main'`, else the whole subject
pub fn branch_name(subject: &str) -> &str {
    subject
        .strip_prefix("Merge branch '")
        .and_then(|rest| rest.split_once('\''))
        .map_or(subject, |(branch, _)| branch)
}

/// Minutes between two moments: exact on the same day, capped at [`MAX_MULTI_DAY_MINUTES`]
/// across days. Rounded up, and never zero.
pub fn gap_minutes(from: DateTime<FixedOffset>, to: DateTime<FixedOffset>) -> u32 {
    let seconds = (to - from).num_seconds().max(0);
    let minutes = u32::try_from((seconds + SECONDS_PER_MINUTE - 1) / SECONDS_PER_MINUTE)
        .unwrap_or(u32::MAX)
        .max(1);
    if from.date_naive() == to.date_naive() {
        minutes
    } else {
        minutes.min(MAX_MULTI_DAY_MINUTES)
    }
}

/// One session per merge, in order; the first is timed from `start` (the root commit)
pub fn sessions(start: DateTime<FixedOffset>, merges: &[Merge]) -> Vec<WorkSession> {
    let mut previous = start;
    let mut sessions = Vec::with_capacity(merges.len());
    for merge in merges {
        sessions.push(WorkSession {
            name: branch_name(&merge.subject).to_owned(),
            date: merge.at.date_naive(),
            minutes: gap_minutes(previous, merge.at),
            note: format!("{} {}", merge.hash, merge.subject),
        });
        previous = merge.at;
    }
    sessions
}
