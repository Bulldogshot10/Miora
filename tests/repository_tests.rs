use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::NaiveDate;
use miora::domain::symptom::{Severity, SymptomCode, SymptomEntry};
use miora::storage::repository::Repository;
use rusqlite::Connection;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn temporary_database_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("miora-repository-{nonce}.sqlite3"))
}

#[test]
fn bleeding_completion_uses_the_explicit_last_day() {
    let path = temporary_database_path();
    let repository = Repository::open(&path).unwrap();
    let start = date(2026, 11, 1);

    repository.add_period_start(start).unwrap();
    repository
        .report_bleeding_status(start, date(2026, 11, 9), false, Some(date(2026, 11, 5)))
        .unwrap();

    assert_eq!(repository.confirmed_bleeding_durations().unwrap(), vec![5]);

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn bleeding_completion_cannot_precede_a_positive_observation() {
    let path = temporary_database_path();
    let repository = Repository::open(&path).unwrap();
    let start = date(2026, 11, 1);

    repository.add_period_start(start).unwrap();
    repository
        .report_bleeding_status(start, date(2026, 11, 5), true, None)
        .unwrap();

    assert!(
        repository
            .report_bleeding_status(start, date(2026, 11, 9), false, Some(date(2026, 11, 4)))
            .is_err()
    );
    repository
        .report_bleeding_status(start, date(2026, 11, 9), false, Some(date(2026, 11, 5)))
        .unwrap();
    assert_eq!(repository.confirmed_bleeding_durations().unwrap(), vec![5]);

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn bleeding_completion_requires_an_explicit_last_day() {
    let path = temporary_database_path();
    let repository = Repository::open(&path).unwrap();
    let start = date(2026, 11, 1);

    repository.add_period_start(start).unwrap();
    assert!(
        repository
            .report_bleeding_status(start, date(2026, 11, 9), false, None)
            .is_err()
    );
    assert!(
        repository
            .confirmed_bleeding_durations()
            .unwrap()
            .is_empty()
    );

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn new_period_start_cannot_overlap_previous_bleeding() {
    let path = temporary_database_path();
    let repository = Repository::open(&path).unwrap();
    let first_start = date(2026, 11, 1);

    repository.add_period_start(first_start).unwrap();
    repository
        .set_period_end(first_start, date(2026, 11, 10))
        .unwrap();

    assert!(repository.add_period_start(date(2026, 11, 8)).is_err());

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn period_end_must_precede_the_next_period_start() {
    let path = temporary_database_path();
    let repository = Repository::open(&path).unwrap();
    let first_start = date(2026, 11, 1);

    repository.add_period_start(first_start).unwrap();
    repository.add_period_start(date(2026, 11, 8)).unwrap();

    assert!(
        repository
            .set_period_end(first_start, date(2026, 11, 8))
            .is_err()
    );

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn daily_checkin_query_includes_days_without_symptoms() {
    let path = temporary_database_path();
    let repository = Repository::open(&path).unwrap();
    let first_day = date(2026, 11, 1);
    let second_day = date(2026, 11, 2);
    let cramps = [SymptomEntry {
        code: SymptomCode::Cramps,
        severity: Severity::Moderate,
    }];

    repository
        .save_daily_checkin(first_day, 3, 4, &cramps)
        .unwrap();
    repository
        .save_daily_checkin(second_day, 4, 3, &[])
        .unwrap();

    let records = repository.daily_checkins_with_symptoms().unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].0, first_day);
    assert_eq!(records[0].1, vec![SymptomCode::Cramps]);
    assert_eq!(records[1].0, second_day);
    assert!(records[1].1.is_empty());

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn migration_imports_legacy_period_starts() {
    let path = temporary_database_path();
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE period_starts (
                    start_date TEXT NOT NULL UNIQUE
                );
                INSERT INTO period_starts (start_date) VALUES ('2026-11-01');
                ",
            )
            .unwrap();
    }

    let repository = Repository::open(&path).unwrap();
    assert_eq!(
        repository.period_start_dates().unwrap(),
        vec![date(2026, 11, 1)]
    );

    drop(repository);
    let _ = std::fs::remove_file(path);
}
