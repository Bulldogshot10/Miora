use std::collections::BTreeMap;

use jiff::civil::Date as NaiveDate;
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

fn ensure_period_start_allowed(
    transaction: &rusqlite::Transaction<'_>,
    start: NaiveDate,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_text = start.to_string();
    let overlaps_previous: bool = transaction.query_row(
        "
        SELECT EXISTS(
            SELECT 1 FROM periods
            WHERE start_date < ?1
              AND (
                  (end_date IS NOT NULL AND end_date >= ?1)
                  OR (
                      last_bleeding_observation IS NOT NULL
                      AND last_bleeding_observation >= ?1
                  )
              )
        )
        ",
        params![start_text],
        |row| row.get(0),
    )?;
    if overlaps_previous {
        return Err(
            "The new period start overlaps a previously recorded bleeding interval.".into(),
        );
    }
    Ok(())
}

fn set_period_end_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<(), Box<dyn std::error::Error>> {
    if end < start {
        return Err("The last bleeding day must be on or after the first day.".into());
    }

    let start_text = start.to_string();
    let stored_observation: Option<Option<String>> = transaction
        .query_row(
            "SELECT last_bleeding_observation FROM periods WHERE start_date = ?1",
            params![start_text],
            |row| row.get(0),
        )
        .optional()?;
    let Some(previous_observation) = stored_observation else {
        return Err("The period start date could not be found.".into());
    };
    if let Some(previous_observation) = previous_observation {
        let previous_observation = previous_observation.parse::<NaiveDate>()?;
        if end < previous_observation {
            return Err(
                "The last bleeding day cannot be earlier than a previously recorded bleeding observation."
                    .into(),
            );
        }
    }

    let next_start: Option<String> = transaction.query_row(
        "SELECT MIN(start_date) FROM periods WHERE start_date > ?1",
        params![start_text],
        |row| row.get(0),
    )?;
    if let Some(next_start) = next_start {
        let next_start = next_start.parse::<NaiveDate>()?;
        if end >= next_start {
            return Err(
                "The last bleeding day must be before the next recorded period start.".into(),
            );
        }
    }

    let changed = transaction.execute(
        "
        UPDATE periods
        SET end_date = ?1,
            last_bleeding_observation = ?1,
            bleeding_end_confirmed = 1
        WHERE start_date = ?2
        ",
        params![end.to_string(), start_text],
    )?;
    if changed == 0 {
        return Err("The period start date could not be found.".into());
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DailyCheckinRecord {
    pub mood_score: i32,
    pub energy_score: i32,
    pub symptoms: Vec<SymptomEntry>,
}

#[derive(Debug, Clone)]
pub struct PeriodBackupRecord {
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub flow_intensity: Option<i32>,
    pub last_observation: Option<NaiveDate>,
    pub end_confirmed: bool,
}

#[derive(Debug, Clone)]
pub struct DailyCheckinBackupRecord {
    pub date: NaiveDate,
    pub mood_score: i32,
    pub energy_score: i32,
    pub symptoms: Vec<SymptomEntry>,
}

pub type DailySymptomHistory = Vec<(NaiveDate, Vec<SymptomCode>)>;

pub struct Repository {
    database: Database,
}

impl Repository {
    pub fn open(path: &std::path::Path, key: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            database: Database::open(path, key)?,
        })
    }

    pub fn add_period_start(&self, date: NaiveDate) -> Result<bool, Box<dyn std::error::Error>> {
        self.add_period_start_with_end(date, None)
    }

    /// Insert a period start and optionally confirm its end in one transaction.
    /// A failed end-date validation rolls back the newly inserted start.
    pub fn add_period_start_with_end(
        &self,
        start: NaiveDate,
        end: Option<NaiveDate>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some(end) = end {
            if end < start {
                return Err("The last bleeding day must be on or after the first day.".into());
            }
        }

        let transaction = self.database.connection().unchecked_transaction()?;
        ensure_period_start_allowed(&transaction, start)?;

        let start_text = start.to_string();
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO periods (start_date) VALUES (?1)",
            params![start_text],
        )? == 1;

        if let Some(end) = end {
            set_period_end_in_transaction(&transaction, start, end)?;
        }

        transaction.commit()?;
        Ok(inserted)
    }

    pub fn set_period_end(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let transaction = self.database.connection().unchecked_transaction()?;
        set_period_end_in_transaction(&transaction, start, end)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn report_bleeding_status(
        &self,
        start: NaiveDate,
        observation_date: NaiveDate,
        still_bleeding: bool,
        last_bleeding_date: Option<NaiveDate>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if observation_date < start {
            return Err("The observation date cannot be before the period start.".into());
        }

        let transaction = self.database.connection().unchecked_transaction()?;
        let start_text = start.to_string();
        let previous: Option<(Option<String>, bool)> = transaction
            .query_row(
                "SELECT last_bleeding_observation, bleeding_end_confirmed
                 FROM periods WHERE start_date = ?1",
                params![start_text],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let Some((previous_observation, end_confirmed)) = previous else {
            return Err("The period start date could not be found.".into());
        };
        if end_confirmed {
            return Err("This period has already been marked as finished.".into());
        }

        if let Some(previous_observation) = previous_observation.as_deref() {
            let previous_observation = previous_observation.parse::<NaiveDate>()?;
            if observation_date < previous_observation {
                return Err("The bleeding observation date cannot move backwards.".into());
            }
        }

        let next_start: Option<String> = transaction.query_row(
            "SELECT MIN(start_date) FROM periods WHERE start_date > ?1",
            params![start_text],
            |row| row.get(0),
        )?;
        if let Some(next_start) = next_start {
            let next_start = next_start.parse::<NaiveDate>()?;
            if observation_date >= next_start {
                return Err(
                    "The bleeding observation must be before the next recorded period start."
                        .into(),
                );
            }
        }

        let (last_bleeding_date, confirmed) = if still_bleeding {
            (observation_date, false)
        } else {
            let last_bleeding_date =
                last_bleeding_date.ok_or("Select the last day you were bleeding.")?;
            if last_bleeding_date < start || last_bleeding_date > observation_date {
                return Err(
                    "The last bleeding day must be between the period start and today's check."
                        .into(),
                );
            }
            if let Some(previous_observation) = previous_observation.as_deref() {
                let previous_observation = previous_observation.parse::<NaiveDate>()?;
                if last_bleeding_date < previous_observation {
                    return Err(
                        "The last bleeding day cannot be earlier than a previously recorded bleeding observation."
                            .into(),
                    );
                }
            }
            (last_bleeding_date, true)
        };

        let changed = transaction.execute(
            "
            UPDATE periods
            SET end_date = ?1,
                last_bleeding_observation = ?2,
                bleeding_end_confirmed = ?3
            WHERE start_date = ?4
            ",
            params![
                last_bleeding_date.to_string(),
                observation_date.to_string(),
                confirmed,
                start_text
            ],
        )?;
        if changed == 0 {
            return Err("The period start date could not be found.".into());
        }

        transaction.commit()?;
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
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        row.map(|(start, end, observation, confirmed)| {
            Ok(PeriodBleedingRecord {
                start_date: start.parse::<NaiveDate>()?,
                end_date: end.map(|value| value.parse::<NaiveDate>()).transpose()?,
                last_observation: observation
                    .map(|value| value.parse::<NaiveDate>())
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
            let start = start.parse::<NaiveDate>()?;
            let end = end.parse::<NaiveDate>()?;
            if end >= start {
                let days = (end - start).get_days() as u32 + 1;
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
            dates.push(row?.parse::<NaiveDate>()?);
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
            .map(|date| date.parse::<NaiveDate>())
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
        let date_text = date.to_string();

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
            dates.push(row?.parse::<NaiveDate>()?);
        }

        Ok(dates)
    }

    pub fn daily_checkin(
        &self,
        date: NaiveDate,
    ) -> Result<Option<DailyCheckinRecord>, Box<dyn std::error::Error>> {
        let date_text = date.to_string();
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

        Ok(Some(DailyCheckinRecord {
            mood_score,
            energy_score,
            symptoms,
        }))
    }

    pub fn daily_checkins_with_symptoms(
        &self,
    ) -> Result<DailySymptomHistory, Box<dyn std::error::Error>> {
        let mut statement = self.database.connection().prepare(
            "
            SELECT checkins.checkin_date, symptoms.symptom_code
            FROM daily_checkins AS checkins
            LEFT JOIN symptoms ON symptoms.checkin_id = checkins.id
            ORDER BY checkins.checkin_date ASC
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;

        let mut grouped: BTreeMap<NaiveDate, Vec<SymptomCode>> = BTreeMap::new();
        for row in rows {
            let (date, code) = row?;
            let date = date.parse::<NaiveDate>()?;
            let entries = grouped.entry(date).or_default();
            if let Some(code) = code {
                let code = SymptomCode::parse_key(&code)
                    .ok_or_else(|| format!("Unknown symptom code in database: {code}"))?;
                entries.push(code);
            }
        }

        Ok(grouped.into_iter().collect())
    }

    /// Export every user-authored record needed to recreate the local tracker.
    pub fn export_backup_snapshot(
        &self,
    ) -> Result<(Vec<PeriodBackupRecord>, Vec<DailyCheckinBackupRecord>), Box<dyn std::error::Error>>
    {
        // Keep every exported table on one WAL snapshot while the widget can
        // write through its own SQLite connection.
        let transaction = self.database.connection().unchecked_transaction()?;
        let periods = {
            let mut statement = transaction.prepare(
                "
                SELECT start_date, end_date, flow_intensity,
                       last_bleeding_observation, bleeding_end_confirmed
                FROM periods
                ORDER BY start_date ASC
                ",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i32>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, bool>(4)?,
                ))
            })?;
            let mut records = Vec::new();
            for row in rows {
                let (start, end, flow, observation, confirmed) = row?;
                records.push(PeriodBackupRecord {
                    start_date: start.parse::<NaiveDate>()?,
                    end_date: end.map(|value| value.parse::<NaiveDate>()).transpose()?,
                    flow_intensity: flow,
                    last_observation: observation
                        .map(|value| value.parse::<NaiveDate>())
                        .transpose()?,
                    end_confirmed: confirmed,
                });
            }
            records
        };

        let checkin_dates = {
            let mut statement = transaction
                .prepare("SELECT checkin_date FROM daily_checkins ORDER BY checkin_date ASC")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            let mut dates = Vec::new();
            for row in rows {
                dates.push(row?.parse::<NaiveDate>()?);
            }
            dates
        };

        let mut checkins = Vec::with_capacity(checkin_dates.len());
        for date in checkin_dates {
            let record = self
                .daily_checkin(date)?
                .ok_or("A daily check-in disappeared during backup export.")?;
            checkins.push(DailyCheckinBackupRecord {
                date,
                mood_score: record.mood_score,
                energy_score: record.energy_score,
                symptoms: record.symptoms,
            });
        }

        transaction.commit()?;
        Ok((periods, checkins))
    }

    /// Replace the current dataset atomically with a fully validated backup.
    pub fn restore_backup_snapshot(
        &self,
        periods: &[PeriodBackupRecord],
        checkins: &[DailyCheckinBackupRecord],
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::collections::BTreeSet;

        let mut ordered_periods = periods.to_vec();
        ordered_periods.sort_by_key(|record| record.start_date);
        let mut period_dates = BTreeSet::new();

        for record in &ordered_periods {
            if !period_dates.insert(record.start_date) {
                return Err("The backup contains duplicate period start dates.".into());
            }
            if record.end_date.is_some_and(|end| end < record.start_date) {
                return Err("A backup period ends before it starts.".into());
            }
            if record
                .last_observation
                .is_some_and(|observation| observation < record.start_date)
            {
                return Err("A backup bleeding observation precedes its period start.".into());
            }
            if let (Some(end), Some(observation)) = (record.end_date, record.last_observation) {
                if observation < end {
                    return Err(
                        "A backup bleeding observation precedes its recorded end date.".into(),
                    );
                }
            }
            if record
                .flow_intensity
                .is_some_and(|value| !(1..=3).contains(&value))
            {
                return Err("A backup period has an invalid flow intensity.".into());
            }
            if record.end_confirmed
                && (record.end_date.is_none() || record.last_observation.is_none())
            {
                return Err(
                    "A confirmed backup period is missing its end or observation date.".into(),
                );
            }
        }

        for pair in ordered_periods.windows(2) {
            let next_start = pair[1].start_date;
            if pair[0].end_date.is_some_and(|end| end >= next_start)
                || pair[0]
                    .last_observation
                    .is_some_and(|observation| observation >= next_start)
            {
                return Err("The backup contains overlapping period or bleeding records.".into());
            }
        }

        let mut checkin_dates = BTreeSet::new();
        for record in checkins {
            if !checkin_dates.insert(record.date) {
                return Err("The backup contains duplicate daily check-in dates.".into());
            }
            if !(1..=5).contains(&record.mood_score) || !(1..=5).contains(&record.energy_score) {
                return Err("A backup check-in has an invalid mood or energy score.".into());
            }
            if record.symptoms.len() > 5 {
                return Err("A backup check-in contains too many symptoms.".into());
            }
            let mut symptom_codes = BTreeSet::new();
            for symptom in &record.symptoms {
                if !symptom_codes.insert(symptom.code.as_str()) {
                    return Err("A backup check-in contains a duplicate symptom.".into());
                }
            }
        }

        let transaction = self.database.connection().unchecked_transaction()?;
        transaction.execute("DELETE FROM symptoms", [])?;
        transaction.execute("DELETE FROM daily_checkins", [])?;
        transaction.execute("DELETE FROM periods", [])?;

        for record in &ordered_periods {
            transaction.execute(
                "
                INSERT INTO periods (
                    start_date, end_date, flow_intensity,
                    last_bleeding_observation, bleeding_end_confirmed
                )
                VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    record.start_date.to_string(),
                    record.end_date.map(|date| date.to_string()),
                    record.flow_intensity,
                    record.last_observation.map(|date| date.to_string()),
                    record.end_confirmed
                ],
            )?;
        }

        for record in checkins {
            transaction.execute(
                "
                INSERT INTO daily_checkins (checkin_date, mood_score, energy_score)
                VALUES (?1, ?2, ?3)
                ",
                params![
                    record.date.to_string(),
                    record.mood_score,
                    record.energy_score
                ],
            )?;
            let checkin_id = transaction.last_insert_rowid();
            for symptom in &record.symptoms {
                transaction.execute(
                    "
                    INSERT INTO symptoms (checkin_id, symptom_code, severity)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![checkin_id, symptom.code.as_str(), symptom.severity.as_i32()],
                )?;
            }
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
