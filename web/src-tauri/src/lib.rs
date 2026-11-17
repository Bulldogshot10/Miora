use std::sync::Mutex;

use chrono::{Local, NaiveDate};
use miora::domain::{
    cycle::{cycle_day, cycle_length_days},
    symptom::{Severity, SymptomCode, SymptomEntry},
};
use miora::engine::{
    cycle_estimator::estimate_next_period,
    statistics::mean,
};
use miora::storage::repository::Repository;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

struct BackendState {
    repository: Mutex<Repository>,
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DailyCheckinInput {
    date: String,
    mood_score: i32,
    energy_score: i32,
    symptoms: Vec<SymptomInput>,
}

#[derive(Debug, Deserialize)]
struct SymptomInput {
    code: String,
    severity: i32,
}

fn dashboard_snapshot(
    repository: &Repository,
) -> Result<DashboardSnapshot, String> {
    let starts = repository
        .period_start_dates()
        .map_err(|error| error.to_string())?;

    let latest = starts.last().copied();
    let today = Local::now().date_naive();

    let intervals: Vec<f64> = starts
        .windows(2)
        .filter_map(|pair| cycle_length_days(pair[0], pair[1]).map(f64::from))
        .filter(|days| (15.0..=60.0).contains(days))
        .collect();

    let prediction = estimate_next_period(&starts);
    let checkins_tracked = repository
        .daily_checkin_count()
        .map_err(|error| error.to_string())?;

    Ok(DashboardSnapshot {
        last_period_start: latest.map(|date| date.format("%Y-%m-%d").to_string()),
        cycle_day: latest.and_then(|start| cycle_day(start, today)),
        expected_next_period_start: prediction
            .as_ref()
            .map(|value| value.expected_start.format("%Y-%m-%d").to_string()),
        earliest_next_period_start: prediction
            .as_ref()
            .map(|value| value.earliest_start.format("%Y-%m-%d").to_string()),
        latest_next_period_start: prediction
            .as_ref()
            .map(|value| value.latest_start.format("%Y-%m-%d").to_string()),
        cycles_tracked: intervals.len(),
        average_cycle_length: mean(&intervals),
        checkins_tracked,
        period_starts: starts
            .iter()
            .map(|date| date.format("%Y-%m-%d").to_string())
            .collect(),
    })
}

#[tauri::command]
fn get_dashboard_snapshot(
    backend: State<'_, BackendState>,
) -> Result<DashboardSnapshot, String> {
    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;
    dashboard_snapshot(&repository)
}

#[tauri::command]
fn log_period_start(
    date: String,
    backend: State<'_, BackendState>,
) -> Result<DashboardSnapshot, String> {
    let parsed = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| "Choose a valid period start date.".to_string())?;

    if parsed > Local::now().date_naive() {
        return Err("A period start date cannot be in the future.".to_string());
    }

    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;

    repository
        .add_period_start(parsed)
        .map_err(|error| error.to_string())?;

    dashboard_snapshot(&repository)
}

#[tauri::command]
fn save_daily_checkin(
    input: DailyCheckinInput,
    backend: State<'_, BackendState>,
) -> Result<DashboardSnapshot, String> {
    let date = NaiveDate::parse_from_str(&input.date, "%Y-%m-%d")
        .map_err(|_| "Choose a valid check-in date.".to_string())?;

    if date > Local::now().date_naive() {
        return Err("A check-in date cannot be in the future.".to_string());
    }

    let mut symptoms = Vec::with_capacity(input.symptoms.len());
    for item in input.symptoms {
        let code = SymptomCode::parse_key(&item.code)
            .ok_or_else(|| format!("Unknown symptom code: {}", item.code))?;

        let severity = match item.severity {
            1 => Severity::Mild,
            2 => Severity::Moderate,
            3 => Severity::Severe,
            _ => return Err("Symptom severity must be between 1 and 3.".to_string()),
        };

        symptoms.push(SymptomEntry { code, severity });
    }

    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;

    repository
        .save_daily_checkin(date, input.mood_score, input.energy_score, &symptoms)
        .map_err(|error| error.to_string())?;

    dashboard_snapshot(&repository)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let repository = Repository::open(&app_data_dir.join("miora.sqlite3"))
                .map_err(|error| std::io::Error::other(error.to_string()))?;

            app.manage(BackendState {
                repository: Mutex::new(repository),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_dashboard_snapshot,
            log_period_start,
            save_daily_checkin
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Miora");
}
