use std::{collections::BTreeMap, path::Path};

use labdessem_core::system::StudyHorizon;
use serde::Deserialize;

use crate::error::IoError;
use crate::readers::read_csv;

const INTERNAL_PERIOD_DURATION_HOURS: f64 = 0.5;

#[derive(Debug, Deserialize)]
struct DurationRow {
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Duracao")]
    duracao: f64,
}

pub fn read_horizon(oper_path: &Path) -> Result<StudyHorizon, IoError> {
    let duration_rows: Vec<DurationRow> = read_csv(oper_path.join("OPER_DURACAO.csv"))?;
    build_horizon(&duration_rows)
}

fn build_horizon(duration_rows: &[DurationRow]) -> Result<StudyHorizon, IoError> {
    if duration_rows.is_empty() {
        return Err(IoError::invalid_data("OPER_DURACAO.csv cannot be empty"));
    }

    let mut periods = BTreeMap::new();
    for row in duration_rows {
        periods.insert(row.periodo, row.duracao);
    }

    let expected_periods = periods.len();
    for period in 1..=expected_periods {
        if !periods.contains_key(&period) {
            return Err(IoError::invalid_data(format!(
                "missing period {period} in OPER_DURACAO.csv"
            )));
        }
    }

    let mut original_period_durations_hours = Vec::with_capacity(expected_periods);
    let mut internal_to_original_period = Vec::new();
    let mut internal_subperiod_index = Vec::new();

    for original_period in 1..=expected_periods {
        let duration = *periods
            .get(&original_period)
            .expect("duration should exist after sequence validation");
        if duration <= 0.0 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} must be positive"
            )));
        }
        let subperiods = duration / INTERNAL_PERIOD_DURATION_HOURS;
        let rounded_subperiods = subperiods.round();
        if (subperiods - rounded_subperiods).abs() > 1e-8 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} must be a multiple of {INTERNAL_PERIOD_DURATION_HOURS} hour"
            )));
        }
        let subperiod_count = rounded_subperiods as usize;
        if subperiod_count == 0 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} is below the internal time step"
            )));
        }

        original_period_durations_hours.push(duration);
        for subperiod in 1..=subperiod_count {
            internal_to_original_period.push(original_period);
            internal_subperiod_index.push(subperiod);
        }
    }

    Ok(StudyHorizon {
        periods: internal_to_original_period.len(),
        period_duration_hours: INTERNAL_PERIOD_DURATION_HOURS,
        original_periods: expected_periods,
        original_period_durations_hours,
        internal_to_original_period,
        internal_subperiod_index,
    })
}
