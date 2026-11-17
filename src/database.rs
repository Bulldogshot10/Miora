use std::path::Path;

use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(path)?;

        connection.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS period_starts (
                id          INTEGER PRIMARY KEY,
                start_date  TEXT NOT NULL UNIQUE,
                logged_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_period_starts_date
                ON period_starts(start_date);
            ",
        )?;

        Ok(Self { connection })
    }

    /// Stores a period's first day. Returns false when the date was already logged.
    pub fn add_period_start(&self, date: NaiveDate) -> Result<bool, rusqlite::Error> {
        let date_text = date.format("%Y-%m-%d").to_string();

        let changed = self.connection.execute(
            "INSERT OR IGNORE INTO period_starts (start_date) VALUES (?1)",
            params![date_text],
        )?;

        Ok(changed == 1)
    }

    pub fn latest_period_start(&self) -> Result<Option<NaiveDate>, Box<dyn std::error::Error>> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT start_date
                 FROM period_starts
                 ORDER BY start_date DESC
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        value
            .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d"))
            .transpose()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_a_period_start_only_once() {
        let database_path =
            std::env::temp_dir().join(format!("miora-test-{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&database_path);

        let database = Database::open(&database_path).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();

        assert!(database.add_period_start(date).unwrap());
        assert!(!database.add_period_start(date).unwrap());
        assert_eq!(database.latest_period_start().unwrap(), Some(date));

        drop(database);
        let _ = std::fs::remove_file(database_path);
    }
}
