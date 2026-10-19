# CMB/CMO Network Pricing Formulation

This document describes the mathematical formulation for computing the Nodal Marginal Cost (CMB — Custo Marginal de Barra) and the Submarket Marginal Operating Cost (CMO — Custo Marginal de Operação) when the electrical network representation is active (`rede=1`).

## Overview

When the network is enabled, power flow constraints affect the dispatch solution and introduce congestion costs. The nodal price at each bus includes both the submarket demand balance dual and a network congestion component derived from the PTDF (Power Transfer Distribution Factor) matrix and the flow constraint duals.

## Mathematical Formulation

### CMB (Custo Marginal de Barra)

The CMB for bus _i_ at period _t_ is computed as (eq. 2.43):

```
CMB(i,t) = λ_DS(Sb_i, t) + Σ [PTDF(i,l) × λ_L(l,t)]  for l = 1..NL
```

Where:
- `λ_DS(Sb_i, t)` is the demand balance dual for the submarket `Sb_i` containing bus _i_ at period _t_
- `PTDF(i,l)` is the Power Transfer Distribution Factor for bus _i_ on branch _l_
- `λ_L(l,t)` is the flow constraint dual for branch _l_ at period _t_
- `NL` is the number of branches with flow constraints

The PTDF matrix is computed as:

```
PTDF = B_diag × A × B_red^(-1)
```

Where:
- `B_diag` is the diagonal matrix of branch susceptances
- `A` is the reduced incidence matrix (excluding the slack bus)
- `B_red` is the reduced bus susceptance matrix (excluding the slack bus row/column)

### Flow Constraint Duals (λ_L)

Each branch with a flow cut has two constraints (upper and lower bounds):

```
Upper: flow_mw ≤ +thermal_limit_mw
Lower: flow_mw ≥ -thermal_limit_mw
```

The flow dual is the sum of both constraint duals:

```
λ_L(l,t) = dual[upper_row] + dual[lower_row]
```

This captures the net congestion price for the branch regardless of flow direction.

### CMO (Custo Marginal de Operação)

The CMO for submarket _s_ at period _t_ is the demand-weighted average of CMBs for all buses in that submarket (eq. 2.44):

```
CMO(s,t) = Σ [CMB(k,t) × d(k,t)] / Σ [d(k,t)]  for k ∈ Sb_s
```

Where:
- `d(k,t)` is the demand at bus _k_ in period _t_ (MW)
- `Sb_s` is the set of buses in submarket _s_

When total demand in a submarket is zero, the simple average of CMBs is used:

```
CMO(s,t) = Σ CMB(k,t) / |Sb_s|  when Σ d(k,t) = 0
```

## Behavior by Configuration

| Configuration | CMB | CMO |
|---------------|-----|-----|
| `rede=0` | Not computed | Equals PiDemanda (λ_DS) |
| `rede=1`, no violations | Equals λ_DS for all buses | Equals PiDemanda |
| `rede=1`, with congestion | λ_DS + network component | Demand-weighted average of CMBs |

## Output Files

### resultado_cmobar.csv (only when rede=1)

| Column | Description |
|--------|-------------|
| Periodo | Original study period |
| CodigoBarra | Bus numeric identifier |
| NomeBarra | Bus name |
| Submercado | Submarket name |
| CMB | Nodal marginal cost in R$/MWh |

### resultado_cmosist.csv

| Column | Description |
|--------|-------------|
| Periodo | Original study period |
| Submercado | Submarket name |
| PiDemanda | Demand balance dual in R$/MWh |
| CMO | Marginal operating cost in R$/MWh |

When `rede=0`, CMO equals PiDemanda. When `rede=1`, CMO is the demand-weighted average of CMBs.

## Implementation Details

### Code Locations

- **PTDF computation**: `crates/labdessem-simulation/src/network/ptdf.rs`
- **Flow dual extraction**: `crates/labdessem-simulation/src/network/dual_extraction.rs`
- **CMB computation**: `crates/labdessem-simulation/src/network/cmb.rs` → `compute_cmb_per_bus()`
- **CMO computation**: `crates/labdessem-simulation/src/network/cmb.rs` → `compute_cmo_from_cmb()`
- **Workflow integration**: `crates/labdessem-simulation/src/stages.rs` → `compute_cmb_cmo_data()`
- **CSV output**: `crates/labdessem-simulation/src/output.rs` → `write_cmb_csv()`

### Sign Conventions

- Duals from HiGHS are returned with the standard LP convention
- Positive λ_L indicates binding flow constraint with associated congestion cost
- PTDF values are signed according to the direction from slack to the bus

### Period Aggregation

Internal periods are aggregated to original study periods using duration-weighted averages:

```
CMB_original = Σ [CMB_internal × duration] / Σ duration
```

## Relationship to DESSEM

This formulation follows the DESSEM methodology for network-aware marginal cost computation, where:
- The LP-CALC-CMO stage includes all flow constraints for correct dual extraction
- The demand balance dual represents the energy component
- The PTDF-weighted flow duals represent the transmission component
