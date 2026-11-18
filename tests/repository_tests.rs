use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use jiff::civil::Date as NaiveDate;
use miora::domain::symptom::{Severity, SymptomCode, SymptomEntry};
use miora::storage::repository::Repository;
use rusqlite::Connection;

const PLAINTEXT_SQLITE_HEADER: [u8; 16] = [
    0x53, 0x51, 0x4c, 0x69, 0x74, 0x65, 0x20, 0x66, 0x6f, 0x72, 0x6d, 0x61, 0x74, 0x20, 0x33, 0x00,
];

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::new(year as i16, month as i8, day as i8).unwrap()
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
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
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
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
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
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
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
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
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
fn period_end_cannot_backdate_before_a_prior_bleeding_observation() {
    let path = temporary_database_path();
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    let start = date(2026, 11, 1);

    repository.add_period_start(start).unwrap();
    repository
        .report_bleeding_status(start, date(2026, 11, 8), true, None)
        .unwrap();

    assert!(repository.set_period_end(start, date(2026, 11, 7)).is_err());
    repository.set_period_end(start, date(2026, 11, 8)).unwrap();
    assert_eq!(repository.confirmed_bleeding_durations().unwrap(), vec![8]);

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn period_end_must_precede_the_next_period_start() {
    let path = temporary_database_path();
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
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
fn new_period_start_cannot_precede_an_active_bleeding_observation() {
    let path = temporary_database_path();
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    let active_start = date(2026, 11, 1);
    repository.add_period_start(active_start).unwrap();
    repository
        .report_bleeding_status(active_start, date(2026, 11, 8), true, None)
        .unwrap();

    assert!(repository.add_period_start(date(2026, 11, 7)).is_err());
    assert!(repository.add_period_start(date(2026, 11, 9)).is_ok());

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn period_start_and_confirmed_end_are_atomic() {
    let path = temporary_database_path();
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    let next_start = date(2026, 11, 8);
    repository.add_period_start(next_start).unwrap();

    assert!(
        repository
            .add_period_start_with_end(date(2026, 11, 1), Some(next_start))
            .is_err()
    );
    assert_eq!(repository.period_start_dates().unwrap(), vec![next_start]);

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn bleeding_observation_cannot_move_backwards_or_cross_next_period() {
    let path = temporary_database_path();
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    let start = date(2026, 11, 1);
    repository.add_period_start(start).unwrap();
    repository
        .report_bleeding_status(start, date(2026, 11, 6), true, None)
        .unwrap();

    assert!(
        repository
            .report_bleeding_status(start, date(2026, 11, 5), true, None)
            .is_err()
    );
    assert_eq!(
        repository
            .latest_period_bleeding_record()
            .unwrap()
            .unwrap()
            .last_observation,
        Some(date(2026, 11, 6))
    );

    repository.add_period_start(date(2026, 11, 8)).unwrap();
    assert!(
        repository
            .report_bleeding_status(start, date(2026, 11, 8), true, None)
            .is_err()
    );

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn confirmed_period_cannot_be_reopened_by_a_stale_status_update() {
    let path = temporary_database_path();
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    let start = date(2026, 11, 1);
    repository.add_period_start(start).unwrap();
    repository
        .report_bleeding_status(start, date(2026, 11, 8), false, Some(date(2026, 11, 5)))
        .unwrap();

    assert!(
        repository
            .report_bleeding_status(start, date(2026, 11, 9), true, None)
            .is_err()
    );
    assert_eq!(repository.confirmed_bleeding_durations().unwrap(), vec![5]);

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn encrypted_backup_snapshot_round_trips_periods_and_checkins() {
    let source_path = temporary_database_path();
    let restored_path = temporary_database_path();
    let source = Repository::open(&source_path, &[0xA5; 32]).unwrap();
    let start = date(2026, 11, 1);
    source.add_period_start(start).unwrap();
    source
        .report_bleeding_status(start, date(2026, 11, 9), false, Some(date(2026, 11, 5)))
        .unwrap();
    let symptoms = vec![SymptomEntry {
        code: SymptomCode::Cramps,
        severity: Severity::Moderate,
    }];
    source
        .save_daily_checkin(date(2026, 11, 3), 4, 2, &symptoms)
        .unwrap();

    let (periods, checkins) = source.export_backup_snapshot().unwrap();
    assert_eq!(periods.len(), 1);
    assert_eq!(periods[0].start_date, start);
    assert_eq!(periods[0].end_date, Some(date(2026, 11, 5)));
    assert_eq!(periods[0].last_observation, Some(date(2026, 11, 9)));
    assert!(periods[0].end_confirmed);
    assert_eq!(checkins.len(), 1);

    let restored = Repository::open(&restored_path, &[0x5A; 32]).unwrap();
    restored.restore_backup_snapshot(&periods, &checkins).unwrap();
    let (restored_periods, restored_checkins) = restored.export_backup_snapshot().unwrap();
    assert_eq!(restored_periods.len(), 1);
    assert_eq!(restored_periods[0].start_date, start);
    assert_eq!(restored_periods[0].end_date, Some(date(2026, 11, 5)));
    assert_eq!(restored_periods[0].last_observation, Some(date(2026, 11, 9)));
    assert!(restored_periods[0].end_confirmed);
    assert_eq!(restored_checkins.len(), 1);
    assert_eq!(restored_checkins[0].mood_score, 4);
    assert_eq!(restored_checkins[0].energy_score, 2);
    assert_eq!(restored_checkins[0].symptoms, symptoms);

    drop(source);
    drop(restored);
    let _ = std::fs::remove_file(source_path);
    let _ = std::fs::remove_file(restored_path);
}

#[test]
fn invalid_backup_restore_rolls_back_without_destroying_existing_records() {
    use miora::storage::repository::PeriodBackupRecord;

    let path = temporary_database_path();
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    let existing = date(2026, 11, 1);
    repository.add_period_start(existing).unwrap();

    let invalid = [PeriodBackupRecord {
        start_date: date(2026, 11, 10),
        end_date: Some(date(2026, 11, 9)),
        flow_intensity: None,
        last_observation: Some(date(2026, 11, 10)),
        end_confirmed: true,
    }];
    assert!(repository.restore_backup_snapshot(&invalid, &[]).is_err());
    assert_eq!(repository.period_start_dates().unwrap(), vec![existing]);

    drop(repository);
    let _ = std::fs::remove_file(path);
}

#[test]
fn daily_checkin_query_includes_days_without_symptoms() {
    let path = temporary_database_path();
    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
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
fn database_is_encrypted_and_rejects_an_incorrect_key() {
    use std::io::Read;

    let path = temporary_database_path();
    let key = [0xA5; 32];
    let wrong_key = [0x5A; 32];

    {
        let repository = Repository::open(&path, &key).unwrap();
        repository.add_period_start(date(2026, 11, 1)).unwrap();
    }

    let mut header = [0_u8; 16];
    std::fs::File::open(&path)
        .unwrap()
        .read_exact(&mut header)
        .unwrap();

    assert_ne!(header, PLAINTEXT_SQLITE_HEADER);

    let raw_connection = Connection::open(&path).unwrap();
    assert!(
        raw_connection
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row
                .get::<_, i64>(0))
            .is_err(),
        "the database must not be readable without its SQLCipher key"
    );
    drop(raw_connection);

    assert!(
        Repository::open(&path, &wrong_key).is_err(),
        "an incorrect key must not open the encrypted database"
    );

    let repository = Repository::open(&path, &key).unwrap();
    assert_eq!(
        repository.period_start_dates().unwrap(),
        vec![date(2026, 11, 1)]
    );
    drop(repository);

    let _ = std::fs::remove_file(&path);
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    let _ = std::fs::remove_file(std::path::PathBuf::from(wal));
    let mut shm = path.as_os_str().to_os_string();
    shm.push("-shm");
    let _ = std::fs::remove_file(std::path::PathBuf::from(shm));
}

#[test]
fn sqlcipher_migration_preserves_schema_user_version() {
    let path = temporary_database_path();
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE periods (
                    id INTEGER PRIMARY KEY,
                    start_date TEXT NOT NULL UNIQUE,
                    end_date TEXT,
                    flow_intensity INTEGER CHECK (flow_intensity BETWEEN 1 AND 3),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    last_bleeding_observation TEXT,
                    bleeding_end_confirmed INTEGER NOT NULL DEFAULT 0
                        CHECK (bleeding_end_confirmed IN (0, 1))
                );
                INSERT INTO periods (start_date) VALUES ('2026-11-01');
                PRAGMA user_version = 2;
                ",
            )
            .unwrap();
    }

    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    assert_eq!(
        repository.period_start_dates().unwrap(),
        vec![date(2026, 11, 1)]
    );
    drop(repository);

    let connection = Connection::open(&path).unwrap();
    let test_key_hex = "a5".repeat(32);
    connection
        .execute_batch(&format!(r#"PRAGMA key = "x'{test_key_hex}'";"#))
        .unwrap();
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(user_version, 2);
    drop(connection);

    let _ = std::fs::remove_file(&path);
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    let _ = std::fs::remove_file(std::path::PathBuf::from(wal));
    let mut shm = path.as_os_str().to_os_string();
    shm.push("-shm");
    let _ = std::fs::remove_file(std::path::PathBuf::from(shm));
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

    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    assert_eq!(
        repository.period_start_dates().unwrap(),
        vec![date(2026, 11, 1)]
    );

    drop(repository);
    let mut header = [0_u8; 16];
    std::fs::File::open(&path)
        .unwrap()
        .read_exact(&mut header)
        .unwrap();
    assert_ne!(header, PLAINTEXT_SQLITE_HEADER);

    let _ = std::fs::remove_file(path);
}

#[test]
fn concurrent_legacy_database_opens_serialize_sqlcipher_migration() {
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

    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
                repository.period_start_dates().unwrap()
            })
        })
        .collect();

    for handle in handles {
        assert_eq!(handle.join().unwrap(), vec![date(2026, 11, 1)]);
    }

    let mut header = [0_u8; 16];
    std::fs::File::open(&path)
        .unwrap()
        .read_exact(&mut header)
        .unwrap();
    assert_ne!(header, PLAINTEXT_SQLITE_HEADER);

    let _ = std::fs::remove_file(&path);
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(sidecar));
    }
}

#[test]
fn migration_refuses_to_replace_plaintext_database_when_wal_checkpoint_is_busy() {
    let path = temporary_database_path();
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                CREATE TABLE period_starts (
                    start_date TEXT NOT NULL UNIQUE
                );
                INSERT INTO period_starts (start_date) VALUES ('2026-11-01');
                ",
            )
            .unwrap();
    }

    let reader = Connection::open(&path).unwrap();
    reader.execute_batch("BEGIN;").unwrap();
    let initial_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM period_starts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(initial_count, 1);

    {
        let writer = Connection::open(&path).unwrap();
        writer
            .execute(
                "INSERT INTO period_starts (start_date) VALUES ('2026-11-29')",
                [],
            )
            .unwrap();
    }

    assert!(
        Repository::open(&path, &[0xA5; 32]).is_err(),
        "a busy WAL checkpoint must abort migration instead of replacing the source database"
    );

    let raw = Connection::open(&path).unwrap();
    let preserved_count: i64 = raw
        .query_row("SELECT COUNT(*) FROM period_starts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(preserved_count, 2);
    drop(raw);

    reader.execute_batch("ROLLBACK;").unwrap();
    drop(reader);

    let repository = Repository::open(&path, &[0xA5; 32]).unwrap();
    assert_eq!(
        repository.period_start_dates().unwrap(),
        vec![date(2026, 11, 1), date(2026, 11, 29)]
    );
    drop(repository);

    let _ = std::fs::remove_file(&path);
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(sidecar));
    }
}

#[test]
fn database_rejects_newer_schema_without_downgrading_or_destroying_data() {
    let path = temporary_database_path();
    let key = [0xA5; 32];
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE future_data (value TEXT NOT NULL);
                INSERT INTO future_data (value) VALUES ('keep-me');
                PRAGMA user_version = 3;
                ",
            )
            .unwrap();
    }

    assert!(Repository::open(&path, &key).is_err());

    let connection = Connection::open(&path).unwrap();
    let key_hex = "a5".repeat(32);
    connection
        .execute_batch(&format!(r#"PRAGMA key = "x'{key_hex}'";"#))
        .unwrap();
    let version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    let value: String = connection
        .query_row("SELECT value FROM future_data", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 3);
    assert_eq!(value, "keep-me");
    drop(connection);

    let _ = std::fs::remove_file(&path);
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(sidecar));
    }
}
