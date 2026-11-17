use chrono::{Datelike, NaiveDate};

use crate::domain::cycle::{cycle_day, cycle_length_days};
use crate::engine::cycle_estimator::estimate_next_period;
use crate::engine::statistics::mean;
use crate::storage::repository::Repository;

#[derive(Debug, Clone, Default)]
pub struct CalendarCell {
    pub label: String,
    /// 0 = ordinary/blank, 1 = period start, 2 = today.
    pub state: i32,
}

#[derive(Debug)]
pub struct AppState {
    pub has_period_data: bool,
    pub cycle_day_label: String,
    pub last_period_label: String,
    pub next_period_label: String,
    pub calendar_month_label: String,
    pub calendar_weeks: Vec<[CalendarCell; 7]>,
    pub history_summary: String,
    pub cycles_tracked: usize,
    pub average_cycle_label: String,
    pub checkins_tracked: usize,
}

impl AppState {
    pub fn load(
        repository: &Repository,
        today: NaiveDate,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let starts = repository.period_start_dates()?;
        let latest = starts.last().copied();
        let prediction = estimate_next_period(&starts);

        let (cycle_day_label, last_period_label) = match latest {
            Some(start) => {
                let day = cycle_day(start, today)
                    .map(|value| format!("Cycle day {value}"))
                    .unwrap_or_else(|| "Period date is in the future".to_string());

                (
                    day,
                    format!("Last period began {}", start.format("%d %b %Y")),
                )
            }
            None => (
                "Not started".to_string(),
                "No period dates logged yet.".to_string(),
            ),
        };

        let next_period_label = prediction
            .map(|value| {
                if value.earliest_start == value.latest_start {
                    format!("Expected around {}", value.expected_start.format("%d %b"))
                } else {
                    format!(
                        "Expected {}–{}",
                        value.earliest_start.format("%d %b"),
                        value.latest_start.format("%d %b")
                    )
                }
            })
            .unwrap_or_else(|| {
                if starts.len() < 3 {
                    "Personal estimates appear after 3 period starts.".to_string()
                } else {
                    "More consistent history is needed for an estimate.".to_string()
                }
            });

        let cycle_lengths: Vec<f64> = starts
            .windows(2)
            .filter_map(|pair| cycle_length_days(pair[0], pair[1]).map(f64::from))
            .filter(|days| (15.0..=60.0).contains(days))
            .collect();

        let average_cycle_label = mean(&cycle_lengths)
            .map(|value| format!("{value:.0} days"))
            .unwrap_or_else(|| "Not enough data".to_string());

        let cycles_tracked = cycle_lengths.len();
        let checkins_tracked = repository.daily_checkin_count()?;

        let history_summary = match starts.len() {
            0 => "No period history yet. Add the first day of a period to begin.".to_string(),
            1 => "1 period start logged. Add future starts to reveal your pattern.".to_string(),
            count => format!(
                "{count} period starts logged across {cycles_tracked} completed cycle intervals."
            ),
        };

        Ok(Self {
            has_period_data: latest.is_some(),
            cycle_day_label,
            last_period_label,
            next_period_label,
            calendar_month_label: today.format("%B %Y").to_string(),
            calendar_weeks: build_calendar_weeks(today, &starts),
            history_summary,
            cycles_tracked,
            average_cycle_label,
            checkins_tracked,
        })
    }
}

fn build_calendar_weeks(today: NaiveDate, period_starts: &[NaiveDate]) -> Vec<[CalendarCell; 7]> {
    let first_day = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .expect("the current year and month always form a valid date");

    let next_month = if today.month() == 12 {
        NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
    }
    .expect("the next month always forms a valid date");

    let days_in_month = (next_month - first_day).num_days() as u32;
    let leading_blanks = first_day.weekday().num_days_from_monday() as usize;
    let mut cells = vec![CalendarCell::default(); 42];

    for day in 1..=days_in_month {
        let date = NaiveDate::from_ymd_opt(today.year(), today.month(), day)
            .expect("day is within the current month");
        let index = leading_blanks + day as usize - 1;
        let is_period_start = period_starts.contains(&date);

        cells[index] = CalendarCell {
            label: day.to_string(),
            state: if date == today {
                2
            } else if is_period_start {
                1
            } else {
                0
            },
        };
    }

    cells.chunks_exact(7).map(|chunk| {
        [
            chunk[0].clone(),
            chunk[1].clone(),
            chunk[2].clone(),
            chunk[3].clone(),
            chunk[4].clone(),
            chunk[5].clone(),
            chunk[6].clone(),
        ]
    }).collect()
}
