mod database;

use std::path::PathBuf;
use std::rc::Rc;

use chrono::{Local, NaiveDate};
use database::Database;
use slint::SharedString;

slint::include_modules!();

fn refresh_dashboard(
    window: &MainWindow,
    database: &Database,
) -> Result<(), Box<dyn std::error::Error>> {
    match database.latest_period_start()? {
        Some(start_date) => {
            let today = Local::now().date_naive();
            let cycle_day = today.signed_duration_since(start_date).num_days() + 1;

            window.set_cycle_day_label(format!("Cycle day {cycle_day}").into());
            window.set_last_period_label(
                format!("Last period started: {}", start_date.format("%Y-%m-%d")).into(),
            );
        }
        None => {
            window.set_cycle_day_label("No period logged yet".into());
            window.set_last_period_label("Add a period start date to begin.".into());
        }
    }

    Ok(())
}

pub fn run(database_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let database = Rc::new(Database::open(&database_path)?);
    let window = MainWindow::new()?;

    refresh_dashboard(&window, &database)?;

    let window_handle = window.as_weak();
    let database_handle = Rc::clone(&database);

    window.on_save_period_start(move |raw_date: SharedString| {
        let input = raw_date.trim();

        let result = match NaiveDate::parse_from_str(input, "%Y-%m-%d") {
            Err(_) => Err("Use the YYYY-MM-DD date format.".to_string()),
            Ok(date) if date > Local::now().date_naive() => {
                Err("A period start date cannot be in the future.".to_string())
            }
            Ok(date) => match database_handle.add_period_start(date) {
                Ok(true) => Ok("Period start saved.".to_string()),
                Ok(false) => Ok("That date is already in your history.".to_string()),
                Err(error) => Err(format!("Could not save the date: {error}")),
            },
        };

        if let Some(window) = window_handle.upgrade() {
            match result {
                Ok(message) => window.set_status_message(message.into()),
                Err(message) => window.set_status_message(message.into()),
            }

            if let Err(error) = refresh_dashboard(&window, &database_handle) {
                window.set_status_message(
                    format!("Could not refresh the dashboard: {error}").into(),
                );
            }
        }
    });

    window.run()?;
    Ok(())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app.clone()).expect("failed to initialize Slint Android backend");

    let Some(private_data_dir) = app.internal_data_path() else {
        eprintln!("Miora could not access the app's internal data directory.");
        return;
    };

    if let Err(error) = run(private_data_dir.join("miora.sqlite3")) {
        eprintln!("Miora failed to start: {error}");
    }
}
