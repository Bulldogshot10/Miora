pub mod app;
pub mod domain;
pub mod engine;
pub mod storage;

use std::path::PathBuf;
use std::rc::Rc;

use chrono::{Local, NaiveDate};
use slint::{ComponentHandle, SharedString};

use app::state::{AppState, CalendarCell};
use domain::symptom::{Severity, SymptomCode, SymptomEntry};
use storage::repository::Repository;

slint::include_modules!();

fn calendar_day(cell: &CalendarCell) -> CalendarDay {
    CalendarDay {
        label: cell.label.clone().into(),
        state: cell.state,
    }
}

fn refresh_window(
    window: &MainWindow,
    repository: &Repository,
    today: NaiveDate,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState::load(repository, today)?;

    window.set_has_period_data(state.has_period_data);
    window.set_cycle_day_label(state.cycle_day_label.into());
    window.set_last_period_label(state.last_period_label.into());
    window.set_next_period_label(state.next_period_label.into());
    window.set_calendar_month_label(state.calendar_month_label.into());
    window.set_history_summary(state.history_summary.into());
    window.set_cycles_tracked(state.cycles_tracked as i32);
    window.set_average_cycle_label(state.average_cycle_label.into());
    window.set_checkins_tracked(state.checkins_tracked as i32);

    let weeks: Vec<CalendarWeek> = state
        .calendar_weeks
        .iter()
        .map(|week| CalendarWeek {
            monday: calendar_day(&week[0]),
            tuesday: calendar_day(&week[1]),
            wednesday: calendar_day(&week[2]),
            thursday: calendar_day(&week[3]),
            friday: calendar_day(&week[4]),
            saturday: calendar_day(&week[5]),
            sunday: calendar_day(&week[6]),
        })
        .collect();

    window.set_calendar_weeks(slint::ModelRc::new(slint::VecModel::from(weeks)));
    Ok(())
}

pub fn run(database_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let repository = Rc::new(Repository::open(&database_path)?);
    let window = MainWindow::new()?;

    window.set_active_tab(app::navigation::Screen::Home.as_index());
    refresh_window(&window, &repository, Local::now().date_naive())?;

    let window_handle = window.as_weak();
    let period_repository = Rc::clone(&repository);

    window.on_save_period_start(move |raw_date: SharedString| {
        let result = NaiveDate::parse_from_str(raw_date.trim(), "%Y-%m-%d");

        let message = match result {
            Err(_) => "Enter a valid date using YYYY-MM-DD.".to_string(),
            Ok(date) if date > Local::now().date_naive() => {
                "A period start date cannot be in the future.".to_string()
            }
            Ok(date) => match period_repository.add_period_start(date) {
                Ok(true) => "Period start saved.".to_string(),
                Ok(false) => "That date is already in your history.".to_string(),
                Err(error) => format!("Could not save the period: {error}"),
            },
        };

        if let Some(window) = window_handle.upgrade() {
            window.set_status_message(message.into());
            if let Err(error) = refresh_window(
                &window,
                &period_repository,
                Local::now().date_naive(),
            ) {
                window.set_status_message(format!("Could not refresh Miora: {error}").into());
            }
        }
    });

    let window_handle = window.as_weak();
    let checkin_repository = Rc::clone(&repository);

    window.on_save_checkin(
        move |mood, energy, cramps, bloating, headache, fatigue, mood_changes| {
            let mut symptoms = Vec::new();

            for (selected, code) in [
                (cramps, SymptomCode::Cramps),
                (bloating, SymptomCode::Bloating),
                (headache, SymptomCode::Headache),
                (fatigue, SymptomCode::Fatigue),
                (mood_changes, SymptomCode::MoodChanges),
            ] {
                if selected {
                    symptoms.push(SymptomEntry {
                        code,
                        severity: Severity::Mild,
                    });
                }
            }

            let today = Local::now().date_naive();
            let message = match checkin_repository.save_daily_checkin(
                today,
                mood,
                energy,
                &symptoms,
            ) {
                Ok(()) => "Today's check-in has been saved.".to_string(),
                Err(error) => format!("Could not save check-in: {error}"),
            };

            if let Some(window) = window_handle.upgrade() {
                window.set_status_message(message.into());
                if let Err(error) = refresh_window(&window, &checkin_repository, today) {
                    window.set_status_message(format!("Could not refresh Miora: {error}").into());
                }
            }
        },
    );

    window.run()?;
    Ok(())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app.clone()).expect("failed to initialize Slint Android backend");

    let Some(private_data_dir) = app.internal_data_path() else {
        eprintln!("Miora could not access the app's private data directory.");
        return;
    };

    if let Err(error) = run(private_data_dir.join("miora.sqlite3")) {
        eprintln!("Miora failed to start: {error}");
    }
}
