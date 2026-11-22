use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};

use jiff::{civil::Date as NaiveDate, Zoned};
use miora::domain::{
    cycle::{cycle_day, cycle_length_days},
    symptom::{Severity, SymptomCode, SymptomEntry},
};
use miora::engine::{
    cycle_estimator::estimate_next_period_at,
    cycle_phase::{estimate_bleeding_duration, estimate_cycle_phase},
    statistics::mean,
    symptom_model::{probability_on_cycle_day, SymptomObservation},
};
use miora::storage::repository::{
    DailyCheckinBackupRecord, PeriodBackupRecord, PeriodBleedingRecord, Repository,
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use zeroize::Zeroizing;

uniffi::setup_scaffolding!();

static DATABASE_KEY: OnceLock<Zeroizing<Vec<u8>>> = OnceLock::new();

struct BackendState {
    repository: Mutex<Repository>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseProbabilitiesDto {
    menstrual: f64,
    follicular: f64,
    ovulation_window: f64,
    luteal: f64,
    uncertain: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CyclePhaseSnapshot {
    code: String,
    label: String,
    cycle_day: u32,
    estimated_cycle_length: u32,
    completed_intervals_used: usize,
    completed_bleeding_durations_used: usize,
    confidence: f64,
    probabilities: PhaseProbabilitiesDto,
    is_personalized: bool,
    cycle_history_available: bool,
    bleeding_history_available: bool,
    description: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SymptomPatternSnapshot {
    code: String,
    label: String,
    probability: f64,
    observations_used: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BleedingPrompt {
    period_start: String,
    check_date: String,
    predicted_bleeding_duration_days: u32,
    completed_bleeding_durations_used: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardSnapshot {
    last_period_start: Option<String>,
    cycle_day: Option<u32>,
    expected_next_period_start: Option<String>,
    earliest_next_period_start: Option<String>,
    latest_next_period_start: Option<String>,
    cycles_tracked: usize,
    average_cycle_length: Option<f64>,
    checkins_tracked: usize,
    period_starts: Vec<String>,
    checkin_dates: Vec<String>,
    phase: Option<CyclePhaseSnapshot>,
    symptom_patterns: Vec<SymptomPatternSnapshot>,
    bleeding_prompt: Option<BleedingPrompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupPeriodInput {
    start_date: String,
    end_date: Option<String>,
    flow_intensity: Option<i32>,
    last_observation: Option<String>,
    end_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupDocument {
    format: String,
    version: u32,
    periods: Vec<BackupPeriodInput>,
    checkins: Vec<DailyCheckinInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct DailyCheckinInput {
    pub date: String,
    pub mood_score: i32,
    pub energy_score: i32,
    pub symptoms: Vec<SymptomInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct SymptomInput {
    pub code: String,
    pub severity: i32,
}

#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum WidgetApiError {
    Message(String),
}

impl std::fmt::Display for WidgetApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for WidgetApiError {}

fn configured_database_key() -> Result<&'static [u8], String> {
    DATABASE_KEY
        .get()
        .map(|key| key.as_slice())
        .ok_or_else(|| "The Android database key has not been initialized.".to_string())
}

#[uniffi::export]
pub fn configure_database_key(key: Vec<u8>) -> Result<(), WidgetApiError> {
    let key = Zeroizing::new(key);
    if key.len() != 32 {
        return Err(WidgetApiError::Message(
            "The database encryption key must contain exactly 32 bytes.".to_string(),
        ));
    }
    match DATABASE_KEY.set(key) {
        Ok(()) => Ok(()),
        Err(candidate) => {
            if DATABASE_KEY
                .get()
                .is_some_and(|existing| existing.as_slice() == candidate.as_slice())
            {
                Ok(())
            } else {
                Err(WidgetApiError::Message(
                    "A different database key is already active in this process.".to_string(),
                ))
            }
        }
    }
}

fn load_daily_checkin_from_repository(
    repository: &Repository,
    date: NaiveDate,
) -> Result<Option<DailyCheckinInput>, String> {
    let Some(checkin) = repository
        .daily_checkin(date)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };

    Ok(Some(DailyCheckinInput {
        date: date.to_string(),
        mood_score: checkin.mood_score,
        energy_score: checkin.energy_score,
        symptoms: checkin
            .symptoms
            .into_iter()
            .map(|entry| SymptomInput {
                code: entry.code.as_str().to_string(),
                severity: entry.severity.as_i32(),
            })
            .collect(),
    }))
}

fn save_daily_checkin_to_repository(
    repository: &Repository,
    input: DailyCheckinInput,
) -> Result<(), String> {
    let date = input
        .date
        .parse::<NaiveDate>()
        .map_err(|_| "Choose a valid check-in date.".to_string())?;

    if date > Zoned::now().date() {
        return Err("A check-in date cannot be in the future.".to_string());
    }

    let mut symptoms = Vec::with_capacity(input.symptoms.len());
    for item in input.symptoms {
        let code = SymptomCode::parse_key(&item.code)
            .ok_or_else(|| format!("Unknown symptom code: {}", item.code))?;

        let severity = Severity::from_i32(item.severity)
            .ok_or_else(|| "Symptom severity must be between 1 and 3.".to_string())?;

        symptoms.push(SymptomEntry { code, severity });
    }

    repository
        .save_daily_checkin(date, input.mood_score, input.energy_score, &symptoms)
        .map_err(|error| error.to_string())
}

#[uniffi::export]
pub fn widget_get_daily_checkin(
    database_path: String,
    date: String,
) -> Result<Option<DailyCheckinInput>, WidgetApiError> {
    let parsed_date = date
        .parse::<NaiveDate>()
        .map_err(|_| WidgetApiError::Message("Choose a valid check-in date.".to_string()))?;
    let key = configured_database_key().map_err(WidgetApiError::Message)?;
    let repository = Repository::open(Path::new(&database_path), key)
        .map_err(|error| WidgetApiError::Message(error.to_string()))?;

    load_daily_checkin_from_repository(&repository, parsed_date).map_err(WidgetApiError::Message)
}

#[uniffi::export]
pub fn widget_save_daily_checkin(
    database_path: String,
    input: DailyCheckinInput,
) -> Result<(), WidgetApiError> {
    let key = configured_database_key().map_err(WidgetApiError::Message)?;
    let repository = Repository::open(Path::new(&database_path), key)
        .map_err(|error| WidgetApiError::Message(error.to_string()))?;

    save_daily_checkin_to_repository(&repository, input).map_err(WidgetApiError::Message)
}

fn confirmed_duration_for_current_period(record: Option<PeriodBleedingRecord>) -> Option<u32> {
    let record = record?;
    if !record.end_confirmed {
        return None;
    }

    let end = record.end_date?;
    if end < record.start_date {
        return None;
    }

    Some((end - record.start_date).get_days() as u32 + 1)
}

fn observed_bleeding_through_cycle_day(
    record: Option<PeriodBleedingRecord>,
    today: NaiveDate,
) -> Option<u32> {
    let record = record?;
    if record.end_confirmed {
        return None;
    }

    let observed_through = record.last_observation?;
    if observed_through < record.start_date || observed_through < today {
        return None;
    }

    Some((observed_through - record.start_date).get_days() as u32 + 1)
}

fn bleeding_prompt_for_today(
    latest_record: Option<PeriodBleedingRecord>,
    completed_durations: &[u32],
    today: NaiveDate,
) -> Option<BleedingPrompt> {
    let record = latest_record?;
    if record.end_confirmed || today < record.start_date {
        return None;
    }

    // Do not ask twice on the same date. A "yes" answer becomes a confirmed
    // observation through today; the next prompt is eligible tomorrow.
    if record.last_observation.is_some_and(|date| date >= today) {
        return None;
    }

    let cycle_day = cycle_day(record.start_date, today)?;
    let posterior = estimate_bleeding_duration(completed_durations);
    let predicted_duration = posterior.median_duration();

    if cycle_day < predicted_duration {
        return None;
    }

    Some(BleedingPrompt {
        period_start: record.start_date.to_string(),
        check_date: today.to_string(),
        predicted_bleeding_duration_days: predicted_duration,
        completed_bleeding_durations_used: posterior.observations(),
    })
}

fn phase_snapshot_for_today(
    starts: &[NaiveDate],
    completed_durations: &[u32],
    latest_record: Option<PeriodBleedingRecord>,
    today: NaiveDate,
) -> Option<CyclePhaseSnapshot> {
    let confirmed_days = confirmed_duration_for_current_period(latest_record);
    let observed_through_day = observed_bleeding_through_cycle_day(latest_record, today);

    let estimate = estimate_cycle_phase(
        starts,
        completed_durations,
        today,
        confirmed_days,
        observed_through_day,
    )?;

    Some(CyclePhaseSnapshot {
        code: estimate.phase.as_str().to_string(),
        label: estimate.phase.label().to_string(),
        cycle_day: estimate.cycle_day,
        estimated_cycle_length: estimate.estimated_cycle_length,
        completed_intervals_used: estimate.completed_intervals_used,
        completed_bleeding_durations_used: estimate.completed_bleeding_durations_used,
        confidence: estimate.confidence,
        probabilities: PhaseProbabilitiesDto {
            menstrual: estimate.probabilities.menstrual,
            follicular: estimate.probabilities.follicular,
            ovulation_window: estimate.probabilities.ovulation_window,
            luteal: estimate.probabilities.luteal,
            uncertain: estimate.probabilities.uncertain,
        },
        is_personalized: estimate.is_personalized,
        cycle_history_available: estimate.cycle_history_available,
        bleeding_history_available: estimate.bleeding_history_available,
        description: estimate.phase.description().to_string(),
    })
}

fn symptom_patterns_for_cycle_day(
    repository: &Repository,
    starts: &[NaiveDate],
    target_cycle_day: u32,
) -> Result<Vec<SymptomPatternSnapshot>, String> {
    if starts.is_empty() {
        return Ok(Vec::new());
    }

    let checkins = repository
        .daily_checkins_with_symptoms()
        .map_err(|error| error.to_string())?;
    let codes = [
        SymptomCode::Cramps,
        SymptomCode::Bloating,
        SymptomCode::Headache,
        SymptomCode::Fatigue,
        SymptomCode::MoodChanges,
    ];

    // Both inputs are ordered by date. Advance one cursor through period starts
    // instead of rescanning the complete history for every check-in.
    let mut start_index = 0;
    let mut observations_by_code: Vec<Vec<SymptomObservation>> =
        (0..codes.len()).map(|_| Vec::with_capacity(checkins.len())).collect();

    for (date, symptoms) in &checkins {
        while start_index + 1 < starts.len() && starts[start_index + 1] <= *date {
            start_index += 1;
        }
        if starts[start_index] > *date {
            continue;
        }

        let cycle_day = (*date - starts[start_index]).get_days() as u32 + 1;
        for (index, code) in codes.iter().enumerate() {
            observations_by_code[index].push(SymptomObservation {
                cycle_day,
                occurred: symptoms.contains(code),
            });
        }
    }

    let mut patterns = Vec::new();
    for (code, observations) in codes.into_iter().zip(observations_by_code) {
        let matching_count = observations
            .iter()
            .filter(|entry| entry.cycle_day == target_cycle_day)
            .count();
        if matching_count == 0 {
            continue;
        }

        if let Some(probability) = probability_on_cycle_day(&observations, target_cycle_day) {
            patterns.push(SymptomPatternSnapshot {
                code: code.as_str().to_string(),
                label: code.label().to_string(),
                probability,
                observations_used: matching_count,
            });
        }
    }

    Ok(patterns)
}
fn dashboard_snapshot(repository: &Repository) -> Result<DashboardSnapshot, String> {
    let starts = repository
        .period_start_dates()
        .map_err(|error| error.to_string())?;
    let latest = starts.last().copied();
    let today = Zoned::now().date();

    let intervals: Vec<f64> = starts
        .windows(2)
        .filter_map(|pair| cycle_length_days(pair[0], pair[1]).map(f64::from))
        .filter(|days| (15.0..=60.0).contains(days))
        .collect();

    let prediction = estimate_next_period_at(&starts, today);
    let checkin_dates = repository
        .daily_checkin_dates()
        .map_err(|error| error.to_string())?;
    let checkins_tracked = checkin_dates.len();
    let latest_record = repository
        .latest_period_bleeding_record()
        .map_err(|error| error.to_string())?;
    let completed_durations = repository
        .confirmed_bleeding_durations()
        .map_err(|error| error.to_string())?;

    let phase = phase_snapshot_for_today(&starts, &completed_durations, latest_record, today);
    let symptom_patterns = match &phase {
        Some(phase) => symptom_patterns_for_cycle_day(repository, &starts, phase.cycle_day)?,
        None => Vec::new(),
    };

    let bleeding_prompt = bleeding_prompt_for_today(latest_record, &completed_durations, today);

    Ok(DashboardSnapshot {
        last_period_start: latest.map(|date| date.to_string()),
        cycle_day: latest.and_then(|start| cycle_day(start, today)),
        expected_next_period_start: prediction
            .as_ref()
            .map(|value| value.expected_start.to_string()),
        earliest_next_period_start: prediction
            .as_ref()
            .map(|value| value.earliest_start.to_string()),
        latest_next_period_start: prediction
            .as_ref()
            .map(|value| value.latest_start.to_string()),
        cycles_tracked: intervals.len(),
        average_cycle_length: mean(&intervals),
        checkins_tracked,
        period_starts: starts.iter().map(|date| date.to_string()).collect(),
        checkin_dates: checkin_dates.iter().map(|date| date.to_string()).collect(),
        phase,
        symptom_patterns,
        bleeding_prompt,
    })
}

#[tauri::command]
fn get_dashboard_snapshot(backend: State<'_, BackendState>) -> Result<DashboardSnapshot, String> {
    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;
    dashboard_snapshot(&repository)
}

#[tauri::command]
fn log_period_start(
    date: String,
    end_date: Option<String>,
    backend: State<'_, BackendState>,
) -> Result<DashboardSnapshot, String> {
    let parsed_start = date
        .parse::<NaiveDate>()
        .map_err(|_| "Choose a valid period start date.".to_string())?;

    let today = Zoned::now().date();
    if parsed_start > today {
        return Err("A period start date cannot be in the future.".to_string());
    }

    let parsed_end = end_date
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<NaiveDate>())
        .transpose()
        .map_err(|_| "Choose a valid last bleeding date.".to_string())?;

    if let Some(end) = parsed_end {
        if end < parsed_start {
            return Err("The last bleeding day must be on or after the first day.".to_string());
        }
        if end > today {
            return Err("The last bleeding day cannot be in the future.".to_string());
        }
    }

    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;

    repository
        .add_period_start_with_end(parsed_start, parsed_end)
        .map_err(|error| error.to_string())?;

    dashboard_snapshot(&repository)
}

#[tauri::command]
fn report_bleeding_status(
    period_start: String,
    date: String,
    still_bleeding: bool,
    last_bleeding_date: Option<String>,
    backend: State<'_, BackendState>,
) -> Result<DashboardSnapshot, String> {
    let start = period_start
        .parse::<NaiveDate>()
        .map_err(|_| "The period start date is invalid.".to_string())?;
    let observation_date = date
        .parse::<NaiveDate>()
        .map_err(|_| "The bleeding check date is invalid.".to_string())?;
    let parsed_last_bleeding_date = last_bleeding_date
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<NaiveDate>())
        .transpose()
        .map_err(|_| "Choose a valid last bleeding date.".to_string())?;
    if !still_bleeding && parsed_last_bleeding_date.is_none() {
        return Err("Select the last day you were bleeding.".to_string());
    }

    let today = Zoned::now().date();

    if observation_date != today {
        return Err("Refresh the dashboard before answering the bleeding check.".to_string());
    }
    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;
    let latest = repository
        .latest_period_bleeding_record()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "No period start is available.".to_string())?;

    if latest.start_date != start {
        return Err("The current period changed. Refresh and try again.".to_string());
    }
    if latest.end_confirmed {
        return Err("This period has already been marked as finished.".to_string());
    }

    repository
        .report_bleeding_status(
            start,
            observation_date,
            still_bleeding,
            parsed_last_bleeding_date,
        )
        .map_err(|error| error.to_string())?;

    dashboard_snapshot(&repository)
}

fn export_backup_document(repository: &Repository) -> Result<BackupDocument, String> {
    let (periods, checkins) = repository
        .export_backup_snapshot()
        .map_err(|error| error.to_string())?;

    Ok(BackupDocument {
        format: "miora-backup-data".to_string(),
        version: 1,
        periods: periods
            .into_iter()
            .map(|record| BackupPeriodInput {
                start_date: record.start_date.to_string(),
                end_date: record.end_date.map(|date| date.to_string()),
                flow_intensity: record.flow_intensity,
                last_observation: record.last_observation.map(|date| date.to_string()),
                end_confirmed: record.end_confirmed,
            })
            .collect(),
        checkins: checkins
            .into_iter()
            .map(|record| DailyCheckinInput {
                date: record.date.to_string(),
                mood_score: record.mood_score,
                energy_score: record.energy_score,
                symptoms: record
                    .symptoms
                    .into_iter()
                    .map(|entry| SymptomInput {
                        code: entry.code.as_str().to_string(),
                        severity: entry.severity.as_i32(),
                    })
                    .collect(),
            })
            .collect(),
    })
}

fn restore_backup_document(
    repository: &Repository,
    backup: BackupDocument,
    today: NaiveDate,
) -> Result<(), String> {
    if backup.format != "miora-backup-data" || backup.version != 1 {
        return Err("This backup format or version is not supported.".to_string());
    }
    if backup.periods.len() > 10_000 || backup.checkins.len() > 100_000 {
        return Err("The backup contains more records than Miora supports.".to_string());
    }

    let mut periods = Vec::with_capacity(backup.periods.len());
    for record in backup.periods {
        let start_date = record
            .start_date
            .parse::<NaiveDate>()
            .map_err(|_| "A backup period has an invalid start date.".to_string())?;
        let end_date = record
            .end_date
            .map(|value| value.parse::<NaiveDate>())
            .transpose()
            .map_err(|_| "A backup period has an invalid end date.".to_string())?;
        let last_observation = record
            .last_observation
            .map(|value| value.parse::<NaiveDate>())
            .transpose()
            .map_err(|_| "A backup period has an invalid bleeding observation date.".to_string())?;

        if start_date > today
            || end_date.is_some_and(|date| date > today)
            || last_observation.is_some_and(|date| date > today)
        {
            return Err("The backup contains a period or bleeding date in the future.".to_string());
        }

        periods.push(PeriodBackupRecord {
            start_date,
            end_date,
            flow_intensity: record.flow_intensity,
            last_observation,
            end_confirmed: record.end_confirmed,
        });
    }

    let mut checkins = Vec::with_capacity(backup.checkins.len());
    for record in backup.checkins {
        let date = record
            .date
            .parse::<NaiveDate>()
            .map_err(|_| "A backup check-in has an invalid date.".to_string())?;
        if date > today {
            return Err("The backup contains a check-in date in the future.".to_string());
        }

        if record.symptoms.len() > 5 {
            return Err("A backup check-in contains too many symptoms.".to_string());
        }
        let mut symptoms = Vec::with_capacity(record.symptoms.len());
        for item in record.symptoms {
            let code = SymptomCode::parse_key(&item.code)
                .ok_or_else(|| format!("Unknown backup symptom code: {}", item.code))?;
            let severity = Severity::from_i32(item.severity)
                .ok_or_else(|| "A backup symptom severity must be between 1 and 3.".to_string())?;
            symptoms.push(SymptomEntry { code, severity });
        }

        checkins.push(DailyCheckinBackupRecord {
            date,
            mood_score: record.mood_score,
            energy_score: record.energy_score,
            symptoms,
        });
    }

    repository
        .restore_backup_snapshot(&periods, &checkins)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn export_backup(backend: State<'_, BackendState>) -> Result<BackupDocument, String> {
    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;
    export_backup_document(&repository)
}

#[tauri::command]
fn restore_backup(
    backup: BackupDocument,
    backend: State<'_, BackendState>,
) -> Result<DashboardSnapshot, String> {
    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;
    restore_backup_document(&repository, backup, Zoned::now().date())?;
    dashboard_snapshot(&repository)
}
#[tauri::command]
fn get_daily_checkin(
    date: String,
    backend: State<'_, BackendState>,
) -> Result<Option<DailyCheckinInput>, String> {
    let parsed = date
        .parse::<NaiveDate>()
        .map_err(|_| "Choose a valid check-in date.".to_string())?;

    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;

    load_daily_checkin_from_repository(&repository, parsed)
}

#[tauri::command]
fn save_daily_checkin(
    input: DailyCheckinInput,
    backend: State<'_, BackendState>,
) -> Result<DashboardSnapshot, String> {
    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;

    save_daily_checkin_to_repository(&repository, input)?;
    dashboard_snapshot(&repository)
}

#[cfg(not(target_os = "android"))]
fn initialize_desktop_database_key() -> Result<(), String> {
    if DATABASE_KEY.get().is_some() {
        return Ok(());
    }

    let encoded = Zeroizing::new(std::env::var("MIORA_DATABASE_KEY").map_err(|_| {
        "Set MIORA_DATABASE_KEY to a persistent 64-character hexadecimal key.".to_string()
    })?);
    if encoded.len() != 64 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(
            "MIORA_DATABASE_KEY must contain exactly 64 hexadecimal characters.".to_string(),
        );
    }

    let mut key = Zeroizing::new(Vec::with_capacity(32));
    for pair in encoded.as_bytes().chunks_exact(2) {
        let pair = std::str::from_utf8(pair)
            .map_err(|_| "MIORA_DATABASE_KEY contains invalid hexadecimal data.".to_string())?;
        key.push(
            u8::from_str_radix(pair, 16)
                .map_err(|_| "MIORA_DATABASE_KEY contains invalid hexadecimal data.".to_string())?,
        );
    }

    DATABASE_KEY
        .set(key)
        .map_err(|_| "The desktop database key was already initialized.".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(not(target_os = "android"))]
            initialize_desktop_database_key().map_err(std::io::Error::other)?;

            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let database_key = DATABASE_KEY.get().ok_or_else(|| {
                std::io::Error::other("Android Keystore database key is not initialized")
            })?;
            let repository =
                Repository::open(&app_data_dir.join("miora.sqlite3"), database_key.as_slice())
                    .map_err(|error| std::io::Error::other(error.to_string()))?;

            app.manage(BackendState {
                repository: Mutex::new(repository),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_dashboard_snapshot,
            log_period_start,
            report_bleeding_status,
            get_daily_checkin,
            save_daily_checkin,
            export_backup,
            restore_backup
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Miora");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::new(year as i16, month as i8, day as i8).unwrap()
    }

    #[test]
    fn encrypted_backup_round_trips_database_through_webcrypto_and_restore() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let source_path = std::env::temp_dir().join(format!("miora-backup-source-{nonce}.sqlite3"));
        let restored_path =
            std::env::temp_dir().join(format!("miora-backup-restored-{nonce}.sqlite3"));

        let source = Repository::open(&source_path, &[0xA5; 32]).unwrap();
        let starts = [
            date(2026, 8, 1),
            date(2026, 8, 29),
            date(2026, 9, 26),
            date(2026, 10, 24),
        ];
        for (index, start) in starts.iter().copied().enumerate() {
            let end = (index == 0).then_some(date(2026, 8, 5));
            source.add_period_start_with_end(start, end).unwrap();
        }
        source
            .report_bleeding_status(
                starts[3],
                date(2026, 10, 28),
                false,
                Some(date(2026, 10, 27)),
            )
            .unwrap();

        let first_symptoms = [
            SymptomEntry {
                code: SymptomCode::Cramps,
                severity: Severity::Moderate,
            },
            SymptomEntry {
                code: SymptomCode::Fatigue,
                severity: Severity::Mild,
            },
        ];
        let second_symptoms = [SymptomEntry {
            code: SymptomCode::Headache,
            severity: Severity::Severe,
        }];
        source
            .save_daily_checkin(date(2026, 8, 3), 4, 2, &first_symptoms)
            .unwrap();
        source
            .save_daily_checkin(date(2026, 10, 25), 2, 5, &second_symptoms)
            .unwrap();

        let original_document = export_backup_document(&source).unwrap();
        let bridge_script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../scripts/test-backup-crypto-roundtrip.mjs");
        let mut child = Command::new("node")
            .arg(&bridge_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Node.js must be installed for the encrypted backup integration test.");
        child
            .stdin
            .take()
            .expect("backup bridge stdin is available")
            .write_all(&serde_json::to_vec(&original_document).unwrap())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "WebCrypto backup bridge failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let encrypted_round_trip: BackupDocument =
            serde_json::from_slice(&output.stdout).unwrap();

        let restored = Repository::open(&restored_path, &[0x5A; 32]).unwrap();
        restore_backup_document(&restored, encrypted_round_trip, Zoned::now().date()).unwrap();
        let restored_document = export_backup_document(&restored).unwrap();

        assert_eq!(
            serde_json::to_value(&restored_document).unwrap(),
            serde_json::to_value(&original_document).unwrap(),
        );

        drop(source);
        drop(restored);
        for path in [&source_path, &restored_path] {
            let _ = std::fs::remove_file(path);
            for suffix in ["-wal", "-shm"] {
                let mut sidecar = path.as_os_str().to_os_string();
                sidecar.push(suffix);
                let _ = std::fs::remove_file(std::path::PathBuf::from(sidecar));
            }
        }
    }

    #[test]
    fn widget_ffi_round_trip_uses_the_same_daily_checkin_repository() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("miora-widget-ffi-{nonce}.sqlite3"));
        let database_path = path.to_string_lossy().into_owned();

        let input = DailyCheckinInput {
            date: "2026-03-03".to_string(),
            mood_score: 4,
            energy_score: 2,
            symptoms: vec![
                SymptomInput {
                    code: "cramps".to_string(),
                    severity: 3,
                },
                SymptomInput {
                    code: "fatigue".to_string(),
                    severity: 1,
                },
            ],
        };

        configure_database_key(vec![0xA5; 32]).unwrap();
        widget_save_daily_checkin(database_path.clone(), input).unwrap();
        let restored = widget_get_daily_checkin(database_path, "2026-03-03".to_string())
            .unwrap()
            .unwrap();

        assert_eq!(restored.mood_score, 4);
        assert_eq!(restored.energy_score, 2);
        assert_eq!(restored.symptoms.len(), 2);
        assert!(restored
            .symptoms
            .iter()
            .any(|entry| { entry.code == "cramps" && entry.severity == 3 }));
        assert!(restored
            .symptoms
            .iter()
            .any(|entry| { entry.code == "fatigue" && entry.severity == 1 }));

        drop(restored);
        let _ = std::fs::remove_file(&path);
        let mut wal = path.as_os_str().to_os_string();
        wal.push("-wal");
        let _ = std::fs::remove_file(std::path::PathBuf::from(wal));
        let mut shm = path.as_os_str().to_os_string();
        shm.push("-shm");
        let _ = std::fs::remove_file(std::path::PathBuf::from(shm));
    }

    #[test]
    fn symptom_inference_partially_pools_positive_and_negative_same_day_checkins() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("miora-symptoms-{nonce}.sqlite3"));
        let repository = Repository::open(&path, &[0xA5; 32]).unwrap();

        let starts = [date(2026, 1, 1), date(2026, 1, 29), date(2026, 2, 26)];
        for start in starts {
            repository.add_period_start(start).unwrap();
        }

        let cramps = [SymptomEntry {
            code: SymptomCode::Cramps,
            severity: Severity::Moderate,
        }];
        repository
            .save_daily_checkin(date(2026, 1, 2), 3, 3, &cramps)
            .unwrap();
        repository
            .save_daily_checkin(date(2026, 1, 3), 3, 3, &cramps)
            .unwrap();
        repository
            .save_daily_checkin(date(2026, 1, 4), 3, 3, &cramps)
            .unwrap();
        repository
            .save_daily_checkin(date(2026, 1, 31), 3, 3, &[])
            .unwrap();
        repository
            .save_daily_checkin(date(2026, 2, 28), 3, 3, &cramps)
            .unwrap();

        let patterns = symptom_patterns_for_cycle_day(&repository, &starts, 3).unwrap();
        let cramps_pattern = patterns
            .iter()
            .find(|pattern| pattern.code == "cramps")
            .unwrap();

        assert_eq!(cramps_pattern.observations_used, 3);
        assert!((cramps_pattern.probability - (2.0 / 3.0)).abs() < 1e-9);

        drop(repository);
        let _ = std::fs::remove_file(path);
    }
}
