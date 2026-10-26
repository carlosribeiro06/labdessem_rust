use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq)]
pub struct InflowHistoryRow {
    pub hydro_id: i32,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub value_m3s: f64,
}
