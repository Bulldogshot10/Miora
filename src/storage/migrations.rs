use rusqlite::{Connection, Result};

const INITIAL_SCHEMA: &str = r#"
CREATE TABLE periods (
    id              INTEGER PRIMARY KEY,
    start_date      TEXT NOT NULL UNIQUE,
    end_date        TEXT,
    flow_intensity  INTEGER CHECK (flow_intensity BETWEEN 1 AND 3),
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_periods_start_date ON periods(start_date);

CREATE TABLE daily_checkins (
    id              INTEGER PRIMARY KEY,
    checkin_date    TEXT NOT NULL UNIQUE,
    mood_score      INTEGER NOT NULL CHECK (mood_score BETWEEN 1 AND 5),
    energy_score    INTEGER NOT NULL CHECK (energy_score BETWEEN 1 AND 5),
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE symptoms (
    checkin_id      INTEGER NOT NULL REFERENCES daily_checkins(id) ON DELETE CASCADE,
    symptom_code    TEXT NOT NULL,
    severity        INTEGER NOT NULL CHECK (severity BETWEEN 1 AND 3),
    PRIMARY KEY (checkin_id, symptom_code)
);
"#;

pub fn migrate(connection: &Connection) -> Result<()> {
    let version: i32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version < 1 {
        let has_legacy_period_table: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'period_starts'
            )",
            [],
            |row| row.get(0),
        )?;

        connection.execute_batch("BEGIN IMMEDIATE;")?;

        if let Err(error) = connection.execute_batch(INITIAL_SCHEMA) {
            let _ = connection.execute_batch("ROLLBACK;");
            return Err(error);
        }

        if has_legacy_period_table
            && let Err(error) = connection.execute_batch(
                "
                INSERT OR IGNORE INTO periods (start_date)
                SELECT start_date FROM period_starts;

                DROP TABLE period_starts;
                ",
            )
        {
            let _ = connection.execute_batch("ROLLBACK;");
            return Err(error);
        }

        connection.execute_batch("PRAGMA user_version = 1; COMMIT;")?;
    }

    Ok(())
}
