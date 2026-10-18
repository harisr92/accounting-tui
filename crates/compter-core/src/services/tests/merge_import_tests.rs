use chrono::{DateTime, FixedOffset, NaiveDate};

use crate::services::merge_import::{
    MAX_MULTI_DAY_MINUTES, Merge, branch_name, gap_minutes, parse_log, sessions,
};

fn at(text: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(text).unwrap()
}

fn merge(hash: &str, time: &str, branch: &str) -> Merge {
    Merge {
        hash: hash.to_owned(),
        at: at(time),
        subject: format!("Merge branch '{branch}' into 'main'"),
    }
}

#[test]
fn same_day_gaps_are_exact() {
    let minutes = gap_minutes(
        at("2026-03-21T11:08:25+05:30"),
        at("2026-03-21T20:02:20+05:30"),
    );
    assert_eq!(minutes, 534);
}

#[test]
fn gaps_across_days_are_capped_at_a_working_day() {
    let minutes = gap_minutes(
        at("2026-03-15T16:26:01+05:30"),
        at("2026-03-21T11:08:25+05:30"),
    );
    assert_eq!(minutes, MAX_MULTI_DAY_MINUTES);
}

#[test]
fn short_gaps_across_midnight_keep_their_length() {
    let minutes = gap_minutes(
        at("2026-05-18T23:00:00+05:30"),
        at("2026-05-19T01:00:00+05:30"),
    );
    assert_eq!(minutes, 120);
}

#[test]
fn partial_minutes_round_up_and_never_reach_zero() {
    let start = at("2026-07-11T14:30:09+05:30");
    assert_eq!(gap_minutes(start, at("2026-07-11T14:30:40+05:30")), 1);
    assert_eq!(gap_minutes(start, start), 1);
}

#[test]
fn the_first_merge_is_timed_from_the_start_commit() {
    let merges = [
        merge(
            "79068db6",
            "2026-03-15T16:26:01+05:30",
            "feat/page-with-glow",
        ),
        merge("2752b39a", "2026-03-15T18:26:01+05:30", "fix/theme-colors"),
    ];
    let sessions = sessions(at("2026-03-14T22:22:49+05:30"), &merges);

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].minutes, MAX_MULTI_DAY_MINUTES);
    assert_eq!(sessions[0].name, "feat/page-with-glow");
    assert_eq!(
        sessions[0].date,
        NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()
    );
    assert_eq!(sessions[1].minutes, 120);
    assert_eq!(sessions[1].hash(), "2752b39a");
}

#[test]
fn log_lines_are_parsed_and_malformed_ones_skipped() {
    let log = "79068db6\t2026-03-15T16:26:01+05:30\tMerge branch 'a' into 'main'\n\
               garbage line\n\
               adefa937\t2026-09-23T22:08:44+05:30\tMerge branch 'b'\n";
    let merges = parse_log(log);

    assert_eq!(merges.len(), 2);
    assert_eq!(merges[1].hash, "adefa937");
    assert_eq!(merges[1].subject, "Merge branch 'b'");
}

#[test]
fn branch_names_come_from_merge_subjects() {
    assert_eq!(branch_name("Merge branch 'feat/x' into 'main'"), "feat/x");
    assert_eq!(branch_name("Merge branch 'fix/y'"), "fix/y");
    assert_eq!(
        branch_name("Merge pull request #3"),
        "Merge pull request #3"
    );
}
