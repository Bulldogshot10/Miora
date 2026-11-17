use std::sync::Mutex;

use chrono::{Local, NaiveDate};
use miora::domain::{
    cycle::{cycle_day, cycle_length_days},
    symptom::{Severity, SymptomCode, SymptomEntry},
};
use miora::engine::{
    cycle_estimator::estimate_next_period,
    cycle_phase::{estimate_bleeding_duration, estimate_cycle_phase},
    statistics::mean,
};
use miora::storage::repository::{PeriodBleedingRecord, Repository};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

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
    description: String,
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
    bleeding_prompt: Option<BleedingPrompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DailyCheckinInput {
    date: String,
    mood_score: i32,
    energy_score: i32,
    symptoms: Vec<SymptomInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SymptomInput {
    code: String,
    severity: i32,
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

    Some((end - record.start_date).num_days() as u32 + 1)
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

    Some((observed_through - record.start_date).num_days() as u32 + 1)
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
        period_start: record.start_date.format("%Y-%m-%d").to_string(),
        check_date: today.format("%Y-%m-%d").to_string(),
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
        },
        is_personalized: estimate.is_personalized,
        description: estimate.phase.description().to_string(),
    })
}

fn dashboard_snapshot(repository: &Repository) -> Result<DashboardSnapshot, String> {
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

    let phase = phase_snapshot_for_today(
        &starts,
        &completed_durations,
        latest_record,
        today,
    );

    let bleeding_prompt =
        bleeding_prompt_for_today(latest_record, &completed_durations, today);

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
        checkin_dates: checkin_dates
            .iter()
            .map(|date| date.format("%Y-%m-%d").to_string())
            .collect(),
        phase,
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
    let parsed_start = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| "Choose a valid period start date.".to_string())?;

    let today = Local::now().date_naive();
    if parsed_start > today {
        return Err("A period start date cannot be in the future.".to_string());
    }

    let parsed_end = end_date
        .filter(|value| !value.is_empty())
        .map(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d"))
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
        .add_period_start(parsed_start)
        .map_err(|error| error.to_string())?;

    if let Some(end) = parsed_end {
        repository
            .set_period_end(parsed_start, end)
            .map_err(|error| error.to_string())?;
    }

    dashboard_snapshot(&repository)
}

#[tauri::command]
fn report_bleeding_status(
    period_start: String,
    date: String,
    still_bleeding: bool,
    backend: State<'_, BackendState>,
) -> Result<DashboardSnapshot, String> {
    let start = NaiveDate::parse_from_str(&period_start, "%Y-%m-%d")
        .map_err(|_| "The period start date is invalid.".to_string())?;
    let observation_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| "The bleeding check date is invalid.".to_string())?;
    let today = Local::now().date_naive();

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
        .report_bleeding_status(start, observation_date, still_bleeding)
        .map_err(|error| error.to_string())?;

    dashboard_snapshot(&repository)
}

#[tauri::command]
fn get_daily_checkin(
    date: String,
    backend: State<'_, BackendState>,
) -> Result<Option<DailyCheckinInput>, String> {
    let parsed = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| "Choose a valid check-in date.".to_string())?;

    let repository = backend
        .repository
        .lock()
        .map_err(|_| "Database lock is unavailable.".to_string())?;

    let Some((mood_score, energy_score, symptoms)) = repository
        .daily_checkin(parsed)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };

    Ok(Some(DailyCheckinInput {
        date,
        mood_score,
        energy_score,
        symptoms: symptoms
            .into_iter()
            .map(|entry| SymptomInput {
                code: entry.code.as_str().to_string(),
                severity: entry.severity.as_i32(),
            })
            .collect(),
    }))
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

        let severity = Severity::from_i32(item.severity)
            .ok_or_else(|| "Symptom severity must be between 1 and 3.".to_string())?;

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
            report_bleeding_status,
            get_daily_checkin,
            save_daily_checkin
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Miora");
}
