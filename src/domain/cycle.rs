use chrono::NaiveDate;

pub fn cycle_day(period_start: NaiveDate, date: NaiveDate) -> Option<u32> {
    let days = date.signed_duration_since(period_start).num_days();
    (days >= 0).then_some(days as u32 + 1)
}

pub fn cycle_length_days(previous_start: NaiveDate, next_start: NaiveDate) -> Option<u32> {
    let days = next_start
        .signed_duration_since(previous_start)
        .num_days();

    (days > 0).then_some(days as u32)
}
