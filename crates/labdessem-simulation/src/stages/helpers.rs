//! Helper functions for simulation stages.
//!
//! Contains utility functions for:
//! - Flow cut management and slack variables
//! - CMO/CMB computation
//! - Infeasibility collection
//! - Model adaptation

use std::collections::{BTreeMap, BTreeSet, HashMap};

use labdessem_core::ids::SubmarketId;
use labdessem_core::system::{OperationalLimitVariable, System};
use labdessem_model::{
    constraints::{ConstraintSense, LinearConstraint, LinearTerm},
    indexing::ColumnOffsets,
    objective::ObjectiveTerm,
    variables::{Variable, VariableDomain},
    Model,
};
use labdessem_network::{
    compute_cmb_per_bus, compute_cmo_from_cmb, constant_load_shift,
    constraints::{build_flow_cut_terms, FlowCutInput, GeneratorEntry, PumpingEntry},
    extract_flow_duals, CmbRecord, DcNetworkModel, FlowCutKey,
};
use labdessem_solver::LpSolution;

use crate::results::{
    AddedFlowCutLineResult, FlowCutMapping, LineFlowResult, NetworkInfeasibilityRecord,
    OperationalLimitInfeasibilityRecord,
};
use crate::workflow::IterationStage;

use super::common::NETWORK_FLOW_SLACK_PENALTY_PER_MW;

/// Convert volume in hm³ to flow in m³/s given duration in hours.
fn volume_hm3_to_flow_m3s(volume_hm3: f64, duration_hours: f64) -> f64 {
    if duration_hours.abs() < 1e-12 {
        return 0.0;
    }
    volume_hm3 * 1_000_000.0 / (duration_hours * 3600.0)
}

/// Build flow cut terms by adapting Model/System to FlowCutInput.
pub(crate) fn flow_terms_for_cut(
    model: &Model,
    system: &System,
    dc_network: &DcNetworkModel,
    branch_idx: usize,
    period: usize,
) -> Vec<LinearTerm> {
    let horizon = system.horizon.periods;
    let flow_conversion = system.horizon.period_duration_hours * 0.0036;

    let mut input = FlowCutInput::new(flow_conversion);

    for (entry_idx, entry) in model.indexing.thermal_unit_entries.iter().enumerate() {
        let plant = &system.thermal_plants[entry.plant_idx];
        let variable = &model.variables.thermal_generation[entry_idx * horizon + period];
        input.add_thermal(GeneratorEntry::new(plant.bus_id, &variable.name));
    }

    for (entry_idx, entry) in model.indexing.hydro_unit_entries.iter().enumerate() {
        let plant = &system.hydro_plants[entry.plant_idx];
        let variable = &model.variables.hydro_generation[entry_idx * horizon + period];
        input.add_hydro(GeneratorEntry::new(plant.bus_id, &variable.name));
    }

    for (entry_idx, entry) in model.indexing.renewable_plant_entries.iter().enumerate() {
        let plant = &system.renewable_plants[entry.plant_idx];
        let variable = &model.variables.renewable_generation[entry_idx * horizon + period];
        input.add_renewable(GeneratorEntry::new(plant.bus_id, &variable.name));
    }

    for (entry_idx, entry) in model.indexing.pumping_plant_entries.iter().enumerate() {
        let plant = &system.pumping_plants[entry.plant_idx];
        let variable = &model.variables.pumping[entry_idx * horizon + period];
        input.add_pumping(PumpingEntry::new(
            plant.bus_id,
            &variable.name,
            plant.specific_consumption_mw_per_m3s,
        ));
    }

    build_flow_cut_terms(dc_network, branch_idx, &input)
        .into_iter()
        .map(|term| LinearTerm {
            variable: term.variable_name,
            coefficient: term.coefficient,
        })
        .collect()
}

pub(crate) fn flow_slack_name(prefix: &str, branch_name: &str, period: usize) -> String {
    format!("{prefix}[branch={branch_name},t={}]", period + 1)
}

pub(crate) fn add_network_flow_slack_variable(model: &mut Model, name: &str) {
    model.variables.network_flow_slack.push(Variable {
        name: name.to_string(),
        lower_bound: 0.0,
        upper_bound: None,
        domain: VariableDomain::Continuous,
        fixed_value: None,
    });
    model.objective.terms.push(ObjectiveTerm {
        variable: name.to_string(),
        coefficient: NETWORK_FLOW_SLACK_PENALTY_PER_MW,
    });
}

/// Extend the cut set with new cuts from violations.
pub(crate) fn extend_cut_set(
    accumulated_cuts: &mut BTreeSet<FlowCutKey>,
    system: &System,
    violations: &[crate::results::FlowViolation],
) -> usize {
    let mut new_cuts = 0;
    for violation in violations {
        let branch_idx = system
            .branches
            .iter()
            .position(|b| b.id == violation.branch_id)
            .expect("branch must exist");
        let cut_key = FlowCutKey {
            branch_idx,
            period: violation.period - 1,
        };
        if accumulated_cuts.insert(cut_key) {
            new_cuts += 1;
        }
    }
    new_cuts
}

/// Add flow cuts to the model.
pub(crate) fn add_flow_cuts(
    model: &mut Model,
    system: &System,
    cut_keys: &BTreeSet<FlowCutKey>,
    dc_network: &DcNetworkModel,
    use_slack_variables: bool,
) -> FlowCutMapping {
    let mut mapping = FlowCutMapping::default();
    let start_row = model.constraints.linear_constraints.len();

    for cut_key in cut_keys {
        let branch = &system.branches[cut_key.branch_idx];
        let rhs_shift = constant_load_shift(system, dc_network, cut_key.branch_idx, cut_key.period);
        let terms = flow_terms_for_cut(
            model,
            system,
            dc_network,
            cut_key.branch_idx,
            cut_key.period,
        );
        let mut upper_terms = terms.clone();
        let mut lower_terms = terms;

        if use_slack_variables {
            let upper_slack_name =
                flow_slack_name("flow_slack_upper", branch.name.as_str(), cut_key.period);
            let lower_slack_name =
                flow_slack_name("flow_slack_lower", branch.name.as_str(), cut_key.period);
            add_network_flow_slack_variable(model, &upper_slack_name);
            add_network_flow_slack_variable(model, &lower_slack_name);

            upper_terms.push(LinearTerm {
                variable: upper_slack_name,
                coefficient: -1.0,
            });
            lower_terms.push(LinearTerm {
                variable: lower_slack_name,
                coefficient: 1.0,
            });
        }

        let upper_row_idx = model.constraints.linear_constraints.len() - start_row;
        model.constraints.linear_constraints.push(LinearConstraint {
            name: format!(
                "flow_cut_upper[branch={},t={}]",
                branch.name,
                cut_key.period + 1
            ),
            terms: upper_terms,
            sense: ConstraintSense::LessOrEqual,
            rhs: branch.thermal_limit_mw + rhs_shift,
        });

        let lower_row_idx = model.constraints.linear_constraints.len() - start_row;
        model.constraints.linear_constraints.push(LinearConstraint {
            name: format!(
                "flow_cut_lower[branch={},t={}]",
                branch.name,
                cut_key.period + 1
            ),
            terms: lower_terms,
            sense: ConstraintSense::GreaterOrEqual,
            rhs: -branch.thermal_limit_mw + rhs_shift,
        });

        mapping.indices.insert(
            (cut_key.branch_idx, cut_key.period),
            (upper_row_idx, lower_row_idx),
        );
    }

    mapping
}

pub(crate) fn add_operational_limit_slacks(model: &mut Model) {
    let mut slack_specs = Vec::<(usize, String, f64)>::new();

    for (constraint_idx, constraint) in model.constraints.linear_constraints.iter().enumerate() {
        if let Some(suffix) = constraint.name.strip_prefix("operational_limit_lower") {
            slack_specs.push((
                constraint_idx,
                format!("operational_limit_slack_lower{suffix}"),
                1.0,
            ));
        } else if let Some(suffix) = constraint.name.strip_prefix("operational_limit_upper") {
            slack_specs.push((
                constraint_idx,
                format!("operational_limit_slack_upper{suffix}"),
                -1.0,
            ));
        }
    }

    for (constraint_idx, slack_name, coefficient) in slack_specs {
        model.variables.operational_limit_slack.push(Variable {
            name: slack_name.clone(),
            lower_bound: 0.0,
            upper_bound: None,
            domain: VariableDomain::Continuous,
            fixed_value: None,
        });
        model.objective.terms.push(ObjectiveTerm {
            variable: slack_name.clone(),
            coefficient: NETWORK_FLOW_SLACK_PENALTY_PER_MW,
        });
        model.constraints.linear_constraints[constraint_idx]
            .terms
            .push(LinearTerm {
                variable: slack_name,
                coefficient,
            });
    }
}

pub(crate) fn collect_network_infeasibilities(
    system: &System,
    cut_keys: &BTreeSet<FlowCutKey>,
    solution: &LpSolution,
    col_offsets: &ColumnOffsets,
    stage: IterationStage,
    iteration: usize,
) -> Vec<NetworkInfeasibilityRecord> {
    let mut records = Vec::new();
    let mut slack_idx = 0;

    for cut_key in cut_keys {
        let branch = &system.branches[cut_key.branch_idx];

        let upper_violation = solution
            .primal
            .get(col_offsets.network_flow_slack + slack_idx)
            .copied()
            .unwrap_or(0.0);
        slack_idx += 1;

        let lower_violation = solution
            .primal
            .get(col_offsets.network_flow_slack + slack_idx)
            .copied()
            .unwrap_or(0.0);
        slack_idx += 1;

        let violation_mw = upper_violation + lower_violation;

        if violation_mw > 1e-8 {
            records.push(NetworkInfeasibilityRecord {
                stage,
                iteration,
                period: cut_key.period + 1,
                branch_name: branch.name.clone(),
                violation_mw,
            });
        }
    }

    records
}

pub(crate) fn collect_operational_limit_infeasibilities(
    system: &System,
    solution: &LpSolution,
    col_offsets: &ColumnOffsets,
    stage: IterationStage,
    iteration: usize,
) -> Vec<OperationalLimitInfeasibilityRecord> {
    let mut aggregated =
        BTreeMap::<(usize, usize, String, OperationalLimitVariable), (f64, f64)>::new();
    let mut slack_idx = 0;

    for limit in &system.operational_limits {
        let plant_code = match limit.target {
            labdessem_core::system::OperationalLimitTarget::ThermalPlant(id) => id.0,
            labdessem_core::system::OperationalLimitTarget::HydroPlant(id) => id.0,
            labdessem_core::system::OperationalLimitTarget::PumpingPlant(id) => id.0,
        };

        for period in limit.start_period..=limit.end_period {
            let lower_violation = solution
                .primal
                .get(col_offsets.operational_limit_slack + slack_idx)
                .copied()
                .unwrap_or(0.0);
            slack_idx += 1;

            let upper_violation = solution
                .primal
                .get(col_offsets.operational_limit_slack + slack_idx)
                .copied()
                .unwrap_or(0.0);
            slack_idx += 1;

            if lower_violation <= 1e-8 && upper_violation <= 1e-8 {
                continue;
            }

            let lower_violation =
                operational_limit_violation_for_output(system, limit.variable, lower_violation);
            let upper_violation =
                operational_limit_violation_for_output(system, limit.variable, upper_violation);
            let entry = aggregated
                .entry((period, plant_code, limit.plant_name.clone(), limit.variable))
                .or_insert((0.0, 0.0));
            entry.0 += lower_violation;
            entry.1 += upper_violation;
        }
    }

    aggregated
        .into_iter()
        .map(
            |((period, plant_code, plant_name, variable), (lower_violation, upper_violation))| {
                OperationalLimitInfeasibilityRecord {
                    stage,
                    iteration,
                    period,
                    plant_code,
                    plant_name,
                    variable,
                    unit: operational_limit_output_unit(variable),
                    lower_violation,
                    upper_violation,
                }
            },
        )
        .collect()
}

fn operational_limit_violation_for_output(
    system: &System,
    variable: OperationalLimitVariable,
    violation: f64,
) -> f64 {
    match variable {
        OperationalLimitVariable::Spillage
        | OperationalLimitVariable::Defluence
        | OperationalLimitVariable::Turbining
        | OperationalLimitVariable::Diversion
        | OperationalLimitVariable::Pumping => {
            volume_hm3_to_flow_m3s(violation, system.horizon.period_duration_hours)
        }
        OperationalLimitVariable::Generation | OperationalLimitVariable::Volume => violation,
    }
}

fn operational_limit_output_unit(variable: OperationalLimitVariable) -> &'static str {
    match variable {
        OperationalLimitVariable::Generation => "MW",
        OperationalLimitVariable::Volume => "hm3",
        OperationalLimitVariable::Spillage
        | OperationalLimitVariable::Defluence
        | OperationalLimitVariable::Turbining
        | OperationalLimitVariable::Diversion
        | OperationalLimitVariable::Pumping => "m3/s",
    }
}

pub(crate) fn summarize_added_flow_cut_lines(
    system: &System,
    cut_keys: &BTreeSet<FlowCutKey>,
    final_line_flows: &[LineFlowResult],
) -> Vec<AddedFlowCutLineResult> {
    let mut periods_by_branch = BTreeMap::<usize, Vec<usize>>::new();
    for cut_key in cut_keys {
        periods_by_branch
            .entry(cut_key.branch_idx)
            .or_default()
            .push(cut_key.period + 1);
    }

    let mut output = Vec::with_capacity(periods_by_branch.len());
    for (branch_idx, active_periods) in periods_by_branch {
        let branch = &system.branches[branch_idx];
        let selected_flow = final_line_flows
            .iter()
            .filter(|line_flow| {
                line_flow.branch_id == branch.id && active_periods.contains(&line_flow.period)
            })
            .max_by(|left, right| left.flow_mw.abs().total_cmp(&right.flow_mw.abs()));

        let flow_mw = selected_flow.map_or(0.0, |line_flow| line_flow.flow_mw);

        output.push(AddedFlowCutLineResult {
            branch_id: branch.id,
            branch_name: branch.name.clone(),
            from_bus_id: branch.from_bus_id,
            to_bus_id: branch.to_bus_id,
            flow_mw,
            limit_mw: branch.thermal_limit_mw,
        });
    }

    output
}

/// Compute CMB and CMO data from LP-CALC-CMO solution.
pub(crate) fn compute_cmb_cmo_data(
    system: &System,
    dc_network: Option<&DcNetworkModel>,
    cmo_solution: &LpSolution,
    all_cuts: &BTreeSet<FlowCutKey>,
) -> (
    Vec<CmbRecord>,
    HashMap<(SubmarketId, usize), f64>,
    Option<FlowCutMapping>,
) {
    let horizon = system.horizon.periods;
    let periods: Vec<usize> = (0..horizon).collect();

    let dc_network = match dc_network {
        Some(dc) if !all_cuts.is_empty() => dc,
        _ => {
            let indexing = labdessem_model::indexing::Indexing::from_system(system);
            let constraint_params = labdessem_model::indexing::ConstraintOffsetParams {
                horizon,
                fpha_enabled: system.fpha_enabled,
                num_fpha_segments: system
                    .hydro_plants
                    .iter()
                    .map(|p| p.fpha_segments.len())
                    .sum(),
                future_cost_enabled: system.future_cost_enabled,
                num_future_cost_cuts: system.future_cost_cuts.len(),
                num_operational_limits: system.operational_limits.len(),
                thermal_uct_enabled: system.thermal_unit_commitment_enabled,
                hydro_uch_enabled: system.hydro_unit_commitment_enabled,
                is_commitment_mode: true,
            };
            let row_offsets = indexing.compute_constraint_offsets(&constraint_params);

            let mut cmo_values = HashMap::new();
            for (submarket_idx, submarket) in system.submarkets.iter().enumerate() {
                for period in 0..horizon {
                    let row_idx = row_offsets.demand_balance + submarket_idx * horizon + period;
                    let dual = cmo_solution.dual.get(row_idx).copied().unwrap_or(0.0);
                    let cmo = dual / system.horizon.period_duration_hours;
                    cmo_values.insert((submarket.id, period), cmo);
                }
            }
            return (Vec::new(), cmo_values, None);
        }
    };

    let num_flow_cut_constraints = all_cuts.len() * 2;
    let base_constraint_count = cmo_solution
        .dual
        .len()
        .saturating_sub(num_flow_cut_constraints);

    let indexing = labdessem_model::indexing::Indexing::from_system(system);
    let constraint_params = labdessem_model::indexing::ConstraintOffsetParams {
        horizon,
        fpha_enabled: system.fpha_enabled,
        num_fpha_segments: system
            .hydro_plants
            .iter()
            .map(|p| p.fpha_segments.len())
            .sum(),
        future_cost_enabled: system.future_cost_enabled,
        num_future_cost_cuts: system.future_cost_cuts.len(),
        num_operational_limits: system.operational_limits.len(),
        thermal_uct_enabled: system.thermal_unit_commitment_enabled,
        hydro_uch_enabled: system.hydro_unit_commitment_enabled,
        is_commitment_mode: true,
    };
    let row_offsets = indexing.compute_constraint_offsets(&constraint_params);

    let mut demand_duals = HashMap::new();
    for (submarket_idx, submarket) in system.submarkets.iter().enumerate() {
        for period in 0..horizon {
            let row_idx = row_offsets.demand_balance + submarket_idx * horizon + period;
            let dual = cmo_solution.dual.get(row_idx).copied().unwrap_or(0.0);
            demand_duals.insert((submarket.id, period), dual);
        }
    }

    let mut mapping = FlowCutMapping::default();
    for (idx, cut_key) in all_cuts.iter().enumerate() {
        let upper_row_idx = idx * 2;
        let lower_row_idx = idx * 2 + 1;
        mapping.indices.insert(
            (cut_key.branch_idx, cut_key.period),
            (upper_row_idx, lower_row_idx),
        );
    }

    let flow_duals = extract_flow_duals(&cmo_solution.dual, &mapping, base_constraint_count);
    let cmb_records = compute_cmb_per_bus(system, dc_network, &demand_duals, &flow_duals, &periods);
    let cmo_values = compute_cmo_from_cmb(&cmb_records, system);

    let cmo_values: HashMap<(SubmarketId, usize), f64> = cmo_values
        .into_iter()
        .map(|(key, val)| (key, val / system.horizon.period_duration_hours))
        .collect();

    (cmb_records, cmo_values, Some(mapping))
}
