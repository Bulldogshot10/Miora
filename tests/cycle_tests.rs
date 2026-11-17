use chrono::NaiveDate;
use miora::domain::cycle::{cycle_day, cycle_length_days};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

#[test]
fn first_period_day_is_cycle_day_one() {
    let start = date(2026, 11, 8);

    assert_eq!(cycle_day(start, start), Some(1));
    assert_eq!(cycle_day(start, date(2026, 11, 17)), Some(10));
}

#[test]
fn cycle_day_is_none_before_period_start() {
    assert_eq!(cycle_day(date(2026, 11, 8), date(2026, 11, 7)), None);
}

#[test]
fn cycle_length_uses_difference_between_start_dates() {
    assert_eq!(
        cycle_length_days(date(2026, 11, 8), date(2026, 12, 6)),
        Some(28)
    );
}
