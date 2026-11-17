use chrono::NaiveDate;
use rusqlite::{OptionalExtension, params};

use crate::domain::symptom::{Severity, SymptomCode, SymptomEntry};
use crate::storage::database::Database;

#[derive(Debug, Clone, Copy)]
pub struct PeriodBleedingRecord {
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub last_observation: Option<NaiveDate>,
    pub end_confirmed: bool,
}

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

    pub fn set_period_end(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if end < start {
            return Err("The last bleeding day must be on or after the first day.".into());
        }

        let start_text = start.format("%Y-%m-%d").to_string();
        let end_text = end.format("%Y-%m-%d").to_string();
        let changed = self.database.connection().execute(
            "
            UPDATE periods
            SET end_date = ?1,
                last_bleeding_observation = ?1,
                bleeding_end_confirmed = 1
            WHERE start_date = ?2
            ",
            params![end_text, start_text],
        )?;

        if changed == 0 {
            return Err("The period start date could not be found.".into());
        }

        Ok(())
    }

    pub fn report_bleeding_status(
        &self,
        start: NaiveDate,
        observation_date: NaiveDate,
        still_bleeding: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if observation_date < start {
            return Err("The observation date cannot be before the period start.".into());
        }

        let start_text = start.format("%Y-%m-%d").to_string();
        let observation_text = observation_date.format("%Y-%m-%d").to_string();

        if still_bleeding {
            let changed = self.database.connection().execute(
                "
                UPDATE periods
                SET end_date = ?1,
                    last_bleeding_observation = ?1,
                    bleeding_end_confirmed = 0
                WHERE start_date = ?2
                ",
                params![observation_text, start_text],
            )?;

            if changed == 0 {
                return Err("The period start date could not be found.".into());
            }
        } else {
            let previous_day = observation_date
                .pred_opt()
                .ok_or("The observation date is out of range.")?;
            let previous_day_text = previous_day.format("%Y-%m-%d").to_string();

            let changed = self.database.connection().execute(
                "
                UPDATE periods
                SET end_date = COALESCE(last_bleeding_observation, ?1),
                    bleeding_end_confirmed = 1
                WHERE start_date = ?2
                ",
                params![previous_day_text, start_text],
            )?;

            if changed == 0 {
                return Err("The period start date could not be found.".into());
            }
        }

        Ok(())
    }

    pub fn latest_period_bleeding_record(
        &self,
    ) -> Result<Option<PeriodBleedingRecord>, Box<dyn std::error::Error>> {
        let row: Option<(String, Option<String>, Option<String>, bool)> = self
            .database
            .connection()
            .query_row(
                "
                SELECT start_date, end_date, last_bleeding_observation, bleeding_end_confirmed
                FROM periods
                ORDER BY start_date DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                    ))
                },
            )
            .optional()?;

        row.map(|(start, end, observation, confirmed)| {
            Ok(PeriodBleedingRecord {
                start_date: NaiveDate::parse_from_str(&start, "%Y-%m-%d")?,
                end_date: end
                    .map(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d"))
                    .transpose()?,
                last_observation: observation
                    .map(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d"))
                    .transpose()?,
                end_confirmed: confirmed,
            })
        })
        .transpose()
    }

    pub fn confirmed_bleeding_durations(&self) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
        let mut statement = self.database.connection().prepare(
            "
            SELECT start_date, end_date
            FROM periods
            WHERE end_date IS NOT NULL AND bleeding_end_confirmed = 1
            ORDER BY start_date ASC
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut durations = Vec::new();
        for row in rows {
            let (start, end) = row?;
            let start = NaiveDate::parse_from_str(&start, "%Y-%m-%d")?;
            let end = NaiveDate::parse_from_str(&end, "%Y-%m-%d")?;
            if end >= start {
                let days = (end - start).num_days() as u32 + 1;
                if (1..=14).contains(&days) {
                    durations.push(days);
                }
            }
        }

        Ok(durations)
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

    pub fn daily_checkin_dates(&self) -> Result<Vec<NaiveDate>, Box<dyn std::error::Error>> {
        let mut statement = self
            .database
            .connection()
            .prepare("SELECT checkin_date FROM daily_checkins ORDER BY checkin_date ASC")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

        let mut dates = Vec::new();
        for row in rows {
            dates.push(NaiveDate::parse_from_str(&row?, "%Y-%m-%d")?);
        }

        Ok(dates)
    }

    pub fn daily_checkin(
        &self,
        date: NaiveDate,
    ) -> Result<Option<(i32, i32, Vec<SymptomEntry>)>, Box<dyn std::error::Error>> {
        let date_text = date.format("%Y-%m-%d").to_string();
        let checkin: Option<(i64, i32, i32)> = self
            .database
            .connection()
            .query_row(
                "
                SELECT id, mood_score, energy_score
                FROM daily_checkins
                WHERE checkin_date = ?1
                ",
                params![date_text],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let Some((checkin_id, mood_score, energy_score)) = checkin else {
            return Ok(None);
        };

        let mut statement = self
            .database
            .connection()
            .prepare("SELECT symptom_code, severity FROM symptoms WHERE checkin_id = ?1")?;
        let rows = statement.query_map(params![checkin_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        let mut symptoms = Vec::new();
        for row in rows {
            let (code, severity) = row?;
            let code = SymptomCode::parse_key(&code)
                .ok_or_else(|| format!("Unknown symptom code in database: {code}"))?;
            let severity = Severity::from_i32(severity)
                .ok_or_else(|| format!("Invalid symptom severity in database: {severity}"))?;
            symptoms.push(SymptomEntry { code, severity });
        }

        Ok(Some((mood_score, energy_score, symptoms)))
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
