use chrono::NaiveDate;
use rusqlite::{OptionalExtension, params};

use crate::domain::symptom::SymptomEntry;
use crate::storage::database::Database;

pub struct Repository {
    database: Database,
}

impl Repository {
    pub fn open(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            database: Database::open(path)?,
        })
    }

    pub fn add_period_start(&self, date: NaiveDate) -> Result<bool, rusqlite::Error> {
        let date_text = date.format("%Y-%m-%d").to_string();
        let changed = self.database.connection().execute(
            "INSERT OR IGNORE INTO periods (start_date) VALUES (?1)",
            params![date_text],
        )?;

        Ok(changed == 1)
    }

    pub fn period_start_dates(&self) -> Result<Vec<NaiveDate>, Box<dyn std::error::Error>> {
        let mut statement = self
            .database
            .connection()
            .prepare("SELECT start_date FROM periods ORDER BY start_date ASC")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

        let mut dates = Vec::new();
        for row in rows {
            dates.push(NaiveDate::parse_from_str(&row?, "%Y-%m-%d")?);
        }

        Ok(dates)
    }

    pub fn latest_period_start(&self) -> Result<Option<NaiveDate>, Box<dyn std::error::Error>> {
        let value: Option<String> = self
            .database
            .connection()
            .query_row(
                "SELECT start_date FROM periods ORDER BY start_date DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        value
            .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d"))
            .transpose()
            .map_err(Into::into)
    }

    pub fn save_daily_checkin(
        &self,
        date: NaiveDate,
        mood_score: i32,
        energy_score: i32,
        symptoms: &[SymptomEntry],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !(1..=5).contains(&mood_score) || !(1..=5).contains(&energy_score) {
            return Err("Mood and energy scores must be between 1 and 5.".into());
        }

        let connection = self.database.connection();
        let transaction = connection.unchecked_transaction()?;
        let date_text = date.format("%Y-%m-%d").to_string();

        transaction.execute(
            "
            INSERT INTO daily_checkins (checkin_date, mood_score, energy_score)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(checkin_date) DO UPDATE SET
                mood_score = excluded.mood_score,
                energy_score = excluded.energy_score
            ",
            params![date_text, mood_score, energy_score],
        )?;

        let checkin_id: i64 = transaction.query_row(
            "SELECT id FROM daily_checkins WHERE checkin_date = ?1",
            params![date_text],
            |row| row.get(0),
        )?;

        transaction.execute(
            "DELETE FROM symptoms WHERE checkin_id = ?1",
            params![checkin_id],
        )?;

        for symptom in symptoms {
            transaction.execute(
                "
                INSERT INTO symptoms (checkin_id, symptom_code, severity)
                VALUES (?1, ?2, ?3)
                ",
                params![checkin_id, symptom.code.as_str(), symptom.severity.as_i32()],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn daily_checkin_count(&self) -> Result<usize, rusqlite::Error> {
        let count: i64 = self.database.connection().query_row(
            "SELECT COUNT(*) FROM daily_checkins",
            [],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }
}
