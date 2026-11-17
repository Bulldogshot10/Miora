use jiff::civil::Date as NaiveDate;

pub fn cycle_day(period_start: NaiveDate, date: NaiveDate) -> Option<u32> {
    let days = (date - period_start).get_days();
    if days >= 0 {
        Some(days as u32 + 1)
    } else {
        None
    }
}

pub fn cycle_length_days(previous_start: NaiveDate, next_start: NaiveDate) -> Option<u32> {
    let days = (next_start - previous_start).get_days();

    (days > 0).then_some(days as u32)
}
