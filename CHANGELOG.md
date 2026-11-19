# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **BREAKING**: `resultado_fpha_computed.csv`: the gamma columns are now unscaled and the model applies `alpha` once
  - Before: `gamma_0`, `gamma_v` and `gamma_q` were already multiplied by `alpha`, `gamma_s` was not, and the model multiplied all four by `alpha` again
  - Now `gamma_0`, `gamma_v`, `gamma_q` and `gamma_s` hold the unscaled fitted planes, and the model applies `alpha` once to all four coefficients
  - The `max_deviation_mw`, `mean_deviation_mw` and `rms_deviation_mw` columns describe `alpha` x plane against the FPH on the fit grid
  - Column names, positions and number format are unchanged
  - Copying the rows into `OPER_FPHA.csv` reproduces the computed run with `USIH = codigo_usina`, `SegFPHA = plane_index + 1`, `Fcorrec = alpha`, `Rhs.mw = gamma_0`, `Varm.mw/hm3 = gamma_v`, `Qtur.mw/m3s = gamma_q` and `Qlat.mw/m3s = gamma_s`
- **BREAKING**: Computed FPHA runs now fail when a hydro plant with generating units cannot be fitted
  - Before: `compute_fpha` printed a `Warning:` and skipped a plant with units whose geometry rows were missing, whose fit parameters were invalid or whose fit failed (every plant, when `CAD/CAD_GEOMETRIA_UHE.csv` was absent), and the plant generated without water
  - Now `compute_fpha` returns `FphaComputeError::PlantsNotFitted` listing every such plant, and the CLI prints `Error: computed FPHA could not be fitted for <n> hydro plant(s) with generating units:` with one `plant <id> (<name>): <reason>` line per plant and exits with code 1 before writing any result
  - Plants without generating units are no longer fitted: they are excluded before any geometry or `inflow_history.parquet` lookup and counted in `plants_skipped`
  - `FphaComputeError::MissingHydro` is unchanged and still fails the run at the first plant missing from `inflow_history.parquet`
- **BREAKING**: Computed FPHA runs now require a tailrace row, `Eficiencia` and `Perda.m` for every hydro plant with generating units
  - Before: a plant without a `CAD/CAD_TAILRACE.csv` row was fitted with a 0 m tailrace level, and an absent or empty `Eficiencia` or `Perda.m` was fitted as efficiency 1.0 and loss 0 m, silently inflating the plant's production function
  - Now `compute_fpha` returns `FphaComputeError::PlantsNotFitted` with `FitFailureReason::MissingInputs` naming every missing input of every affected plant, and the CLI exits with code 1 before writing any result
  - An explicit `Perda.m = 0` stays valid; only an absent column or an empty cell counts as missing
  - `FitFailureReason` gained `MissingInputs(Vec<FphaInput>)` and `FphaComputeError` gained `MissingFphaInputs`
  - EngMax is unchanged and keeps its defaults for missing efficiency, loss and tailrace
- **BREAKING**: Precomputed FPHA decks are rejected when `OPER_FPHA.csv` has rows for a hydro plant without generating units
  - Before: a plant without rows in `CAD/CAD_CONJ_UHE.csv` kept its `OPER_FPHA.csv` rows, and the model built FPHA constraints for it with no generation or turbining terms
  - Now the reader fails with `OPER_FPHA.csv has FPHA rows for hydro plant(s) without generating units in CAD_CONJ_UHE.csv: <code> (<name>), ...`, listing every affected plant in ascending code order, and the CLI exits with code 1 before writing any result
  - A plant without generating units and without `OPER_FPHA.csv` rows reads as before; computed mode is unchanged
  - To migrate, delete the `OPER_FPHA.csv` rows of the listed plants or add their generating units to `CAD_CONJ_UHE.csv`
- **BREAKING**: The FPHA geometry readers reject non-finite values and duplicate rows
  - Before: `CAD/CAD_GEOMETRIA_UHE.csv`, `CAD/CAD_TAILRACE.csv` and `CAD/CAD_FPHA_CONFIG.csv` accepted `NaN`, `inf` and `-inf` in every numeric column, a second row for the same plant in `CAD_TAILRACE.csv` or `CAD_FPHA_CONFIG.csv` silently replaced the first, and `CAD_GEOMETRIA_UHE.csv` kept two points of one plant with the same `volume.hm3`
  - Now each reader returns `IoError::InvalidData` with `plant <id> has non-finite <column> <value> at line <n> in <file>`, `plant <id> has duplicate rows at lines <a> and <b> in <file>` or `plant <id> has duplicate volume <v> at lines <a> and <b> in CAD_GEOMETRIA_UHE.csv`
  - `CAD_GEOMETRIA_UHE.csv` and `CAD_TAILRACE.csv` are read for every study, so a bad value fails the study read and the CLI exits with code 1 before writing any result; a bad `CAD_FPHA_CONFIG.csv` fails `compute_fpha`
  - Decks with finite values and one row per plant, such as `examples/24Barras`, read as before
  - To migrate, replace each non-finite cell with a finite value and delete the repeated row or point
- **BREAKING**: Computed FPHA fits with the MM defaults and `CAD/CAD_FPHA_CONFIG.csv` takes effect
  - Before: every computed fit used a 6 x 6 volume-flow grid, and no field of the file changed the fit: `num_planes` was copied into the plane limit, which no production code enforced, and `tolerance`, `min_volume_pct` and `max_volume_pct` were read and dropped
  - Now every computed fit starts from 5 volume points, 5 flow points, at most 10 planes and an angle reduction tolerance of 1.0 degrees, so the planes, `alpha` and `resultado_fpha_computed.csv` of every computed run change, with or without the file
  - A row of `CAD_FPHA_CONFIG.csv` changes only its plant: `num_planes` is a plane limit, `tolerance` the angle tolerance in degrees, and `min_volume_pct` / `max_volume_pct` the fractions of the `[Vmin.hm3, Vmax.hm3]` interval of `CAD_UHE.csv` the fit covers (an absent bound keeps 0 or 1)
  - The plane limit is now enforced: the angle reduction is repeated with a doubled tolerance, always from the original plane set and at most 32 times, until the plant has at most `num_planes` planes; when the limit cannot be reached the fit fails with `hydro <name>: cannot reduce to <n> planes (<k> remain after the last tolerance)` and the plant is listed in `PlantsNotFitted`; `alpha` is computed before the reduction and does not change, the fit deviation does
  - A window outside the geometry table of its plant fails the fit and the plant is listed in `PlantsNotFitted`; a window configured for a plant with a single-point geometry table is ignored and the CLI prints `Warning: plant <id> (<name>) has a single-point table in CAD/CAD_GEOMETRIA_UHE.csv, so min_volume_pct <a> and max_volume_pct <b> in CAD/CAD_FPHA_CONFIG.csv are ignored`
  - The `CAD_FPHA_CONFIG.csv` reader now returns `IoError::InvalidData` with `plant <id> has num_planes 0 (must be at least 1) at line <n> in CAD_FPHA_CONFIG.csv` or `plant <id> has min_volume_pct <a> not below max_volume_pct <b> at line <n> in CAD_FPHA_CONFIG.csv` (the latter only when both bounds are given), and `compute_fpha` fails with it
  - To migrate, set `num_planes` to at least 1 and `min_volume_pct` below `max_volume_pct` in each row, raise `num_planes` for a plant whose fit reports that the limit cannot be reached, and set `tolerance` where the previous plane count must be kept
- **BREAKING**: FPHA rows read the period-mean volume instead of the end-of-period volume (audit FP-06, ADR-030)
  - Before: the `hydro_fpha` row of a period carried `alpha * gamma_v` on the end-of-period volume only, while the EngMax head volume and the Libs form of the row use the period mean
  - Now every `hydro_fpha` row, computed and precomputed, bounds generation by the planes at the period-mean volume `(V[t] + V[t+1]) / 2`: `alpha * gamma_v / 2` sits on each of the volume slots `t` and `t + 1`, and in the first period (`t=1`), where slot 0 is the fixed initial volume, the row has no slot-0 term and `alpha * gamma_v / 2 * VolInic` moves to the right-hand side
  - Row names, sense and row count are unchanged, and the plants with `gamma_v` 0 (ITUTINGA and FUNIL-GRANDE in `examples/24Barras`) are unaffected
  - `Varm.mw/hm3` of a precomputed `OPER_FPHA.csv` is now a coefficient on the period-mean volume; the columns of `resultado_fpha_computed.csv` and the fit are unchanged
  - `GeracaoMaxPontoMW` follows the period-mean volume; `VolumeHM3` is still the end-of-period volume
  - Measured on `examples/24Barras` before -> after (R$): `O_c` (computed) 2420590.3195 -> 2420558.7522, `O_alpha` (precomputed, `Fcorrec = alpha`) 2420591.0652 -> 2420559.4979, `O_one` (precomputed, `Fcorrec = 1`) 2374998.0326 -> 2374967.2890; the release `FPHA: computed` run `Objective:` 2420590.3195 -> 2420558.7522
  - Maximum static envelope difference `|E(V_mean) - E(V_end)|` along the solved trajectory, before -> after: CAMARGOS 0, ITUTINGA 0, FUNIL-GRANDE 0, FURNAS 0.011043 -> 0.011038 MW (period 9); the maximum change of `GeracaoMaxPontoMW` between the two runs is 0.011084 MW (FURNAS, period 9) and 0 for the other plants

### Fixed
- Computed FPHA applied `alpha` twice to `gamma_0`..`gamma_q` and only once to `gamma_s` (audit FP-02, FP-08); the LP now applies `alpha` once to all four coefficients
- A hydro plant with generating units and no fitted FPHA planes was skipped with a warning and generated without water (audit HI-03, FP-01, NU-04); the run now fails naming the plant and the reason
- Computed FPHA silently fitted a plant with a missing tailrace row as a 0 m tailrace level, and a missing `Eficiencia` or `Perda.m` as efficiency 1.0 and loss 0 m, inflating the production function (audit FP-R02); the run now fails naming the plant and every missing input
- Precomputed FPHA rows for a hydro plant without generating units were kept silently and became FPHA constraints with no generation term, hiding an inconsistent deck (audit FP-07); the reader now rejects the deck naming every such plant
- The FPHA geometry readers let `NaN` and infinities through (the sign checks are false for `NaN`), sorted the volume table in an undefined order for `NaN`, and silently kept the last of two rows for the same plant, so a `NaN` water level reached the fit or the net-head model (audit EM-04, FP-R01); the readers now reject the value or the duplicate naming the file, plant, column and line
- `CAD/CAD_FPHA_CONFIG.csv` had no effect on the fit: `num_planes` was copied but never enforced, `tolerance`, `min_volume_pct` and `max_volume_pct` were dropped, and the grid was fixed at 6 x 6 (audit FP-05); each field now changes the fit of its plant and the defaults follow the MM
- `read_vha_geometry` reported the duplicate-volume error of a hash-ordered plant when several plants had one; it now reports the lowest plant code

## [0.12.0] - 2026-11-05

### Added
- `EngMaxOutcome` and `EngMaxDiagnostic` in `labdessem-simulation`, and the EngMax consistency diagnostic
  - `IterativeSimulationResult.engmax` carries the bound used, the bound recomputed from the final solution, and the diagnostic
  - The diagnostic is logged once and echoed as `EngMax:` lines in the CLI Summary on stderr
- `ModelError::MissingEngMaxBound { plant, period }` in `labdessem-model`
- EngMax infeasibility diagnosis: one EngMax-free re-solve, reported as `SimulationError::EngMaxInfeasible` with the exceeded `engmax[p,t]` rows
- `ConstraintOffsetParams::from_system(system, solve_mode)`, which also implements `PartialEq, Eq`
- `TurbineType::from_deck_field`
- CLI log subscriber (`tracing-subscriber`) on stderr: WARN by default, `RUST_LOG` overrides the level

### Changed
- **BREAKING**: `resultado_hidreletricas.csv`: `EngMaxHM3`/`EngMaxM3s` now report the plant bound that built the final LP-CALC-CMO model
  - Before: the CLI recomputed EngMax from the final LP-CALC-CMO solution, unit rows held per-unit values and the `99;99` row their sum
  - Now the columns hold the plant value per MM eq. 9.34, filled only on `99;99` rows and empty on unit rows and when EngMax is off
  - The value is raw and may exceed `TurbinamentoMaxHM3`/`TurbinamentoMaxM3s` on the same row
  - `EngMaxHM3` is summed over the internal periods of the original period, and `EngMaxM3s` is that sum over the original period's duration
  - Column names, positions and number format are unchanged
- **BREAKING**: Public Rust API of the EngMax correction
  - Removed `labdessem_simulation::initialize_engmax_bounds` and `labdessem_simulation::recalculate_engmax_bounds`
  - `EngMaxBounds::get(plant, unit, period)` and `set(plant, unit, period, value)` became `get(plant, period)` and `set(plant, period, value)`
  - `ConstraintSet::extend_with_engmax(.., engmax_bounds, engmax_enabled: bool)` became `extend_with_engmax(.., engmax_bounds) -> Result<(), ModelError>`
  - `build_engmax_constraints(.., engmax_bounds, engmax_enabled) -> Vec<LinearConstraint>` became `build_engmax_constraints(.., engmax_bounds) -> Result<Vec<LinearConstraint>, ModelError>`
  - `write_all_results` and `write_hydro_csv` lost the `engmax_enabled` parameter; `Some(bounds)` enables the EngMax columns
  - `SimulationError` gained `Model(ModelError)`, `PartialEffectiveHead { plant, groups }`, `Geometry(FphaFittingError)` and `EngMaxInfeasible { stage, iteration, rows }`, and `TurbineType` gained `Pelton`; both enums are exhaustive, so external exhaustive matches break
  - `IterativeSimulationResult` gained the public field `engmax: Option<EngMaxOutcome>`, which breaks external struct literals
- EngMax is one plant-level row per plant with units and per period, `engmax[p=<plant>,t=<period>]`, over the sum of unit turbining; `Q_ef` is the sum and `h_ef` the unit-count-weighted mean
- EngMax bounds are recalculated after every solve (LP cut loop, MILP, LP-FIXED, LP-CALC-CMO), with or without network; the first solve uses the initial volume
- The tailrace flow is the previous solve's plant outflow (turbining plus spillage, in m³/s); spillage is always included and lateral influences are not modelled; the first solve uses the plant maximum turbining with no spillage
- Effective head: `Hef.m <= 0` on some but not all units of a plant is an initialization error (`SimulationError::PartialEffectiveHead`); a plant with `Hef.m <= 0` on every unit is inert (EngMax = `Q_ef`); non-positive heads are counted in one summary warning per bound computation; `calculate_max_swallowing` is now pure and no longer logs
- `Turbina` code 3 maps to Pelton (alpha 0.5, inferred from the deck); unknown codes warn once per plant
- The VHA geometry is built once per run; a malformed VHA table with EngMax on is an initialization error (`SimulationError::Geometry`)

### Fixed
- EngMax is no longer capped at `Q_ef` by the net head (c54620f); EngMax now exceeds `Q_ef` when the net head is above `h_ef`
- The head volume now uses the previous solve's period average `(V[t] + V[t+1]) / 2`, fixing an off-by-one that read `volume[t-1]`
- A missing EngMax bound is now an error in the model build, the diagnostic and the CSV writer; before, the row was silently omitted and the CSV writer wrote 0.0

## [0.11.0] - 2026-10-27

### Added
- **Dynamic Maximum Swallowing (EngMax)**: Head-dependent turbining limits for hydro units
  - New `EngMax` flag in `study_config.json` (0 = disabled, 1 = enabled)
  - Net head calculation from VHA geometry table and tailrace polynomial
  - Formula: `Q_max = Q_ef × (h_l / h_ef)^α` where α = 0.5 (Francis) or 0.2 (Kaplan)
  - Inter-stage recalculation using reservoir volumes from optimization solution
  - New geometry input files: `CAD_GEOMETRIA_UHE.csv` and `CAD_TAILRACE.csv`
  - EngMax columns in `resultado_hidreletricas.csv`: `EngMaxHM3`, `EngMaxM3s`
  - Fallback to effective head when geometry data is unavailable
- New `calculate_max_swallowing()` function in `labdessem-fpha`
- New `EngMaxBounds` storage in `labdessem-core`
- New `engmax.rs` module in `labdessem-simulation` for bounds initialization and recalculation
- Design document: `docs/design/engmax-formulation.md`
- **New crate `labdessem-fpha`**: FPHA (Função de Produção Hidrelétrica Aproximada) fitting pipeline
  - Computes hyperplanes from VHA geometry using Qhull convex hull
  - Pipeline stages: hull fitting → α correction → γₛ secant → plane reduction → deviation metrics
  - Supports run-of-river plants with single-point VHA tables
  - New `fpha_fit` example binary for standalone fitting
- **New crate `labdessem-network`**: DC network flow computation extracted from simulation
  - PTDF matrix computation and incidence matrices
  - Flow cut constraint generation
  - CMB/CMO dual extraction algorithms
  - Linear algebra module for matrix operations
- **New crate `labdessem-uc`**: Solver-agnostic unit commitment formulation
  - Thermal UC: commitment, startup, shutdown variables and constraints
  - Hydro UC: commitment variables and channeling constraints
  - Extracted from `labdessem-model` for better modularity
- **FPHA computed mode** (`FPHA = "computed"`): compute FPHA hyperplanes at runtime
  - Reads `CAD/CAD_GEOMETRIA_UHE.csv` (VHA geometry table)
  - Reads `CAD/CAD_FPHA_CONFIG.csv` (optional fitting parameters per plant)
  - Writes `resultado_fpha_computed.csv` with fitted hyperplanes
- **New example case `caso_sin_computed`**: Full SIN case for FPHA computed mode testing
- **I/O orchestrator**: centralized output writing in `labdessem-io`

### Changed
- `HydroPlant` struct now includes `volume_level_table` and `tailrace_coefficients` fields
- Hydro reader loads geometry data from optional CSV files
- Simulation orchestrator recalculates EngMax bounds after each major solve stage
- Model constraints include EngMax when enabled via `extend_with_engmax()`
- **I/O centralization**: migrated all CSV writers from `labdessem-simulation` to `labdessem-io`
  - Hydro, thermal, renewable, pumping, network, and infeasibility writers
  - Unified FPHA I/O with column aliases and deviation columns
- **Simulation refactoring**: decomposed monolithic `stages.rs` into `stages/` module
  - `common.rs`, `helpers.rs`, `orchestration.rs`, `solve.rs`
- Network flow logic migrated to `labdessem-network` crate
- Legacy network code removed from `labdessem-simulation`

### Fixed
- FPHA computed mode loading and unit conversion
- Run-of-river plant handling in FPHA fitting

## [0.9.0] - 2026-10-19

### Added
- **Nodal Marginal Cost (CMB)**: Compute bus-level marginal costs when network is enabled
  - CMB formula: `λ_DS + Σ(PTDF × λ_L)` using demand balance dual and flow constraint duals
  - New `compute_cmb_per_bus()` function in `labdessem-simulation`
  - New `resultado_cmobar.csv` output with columns: Periodo, CodigoBarra, NomeBarra, Submercado, CMB
- **Submarket Marginal Operating Cost (CMO)**: Demand-weighted average of CMBs per submarket
  - CMO formula: `Σ(CMB × demand) / Σ(demand)` for buses in each submarket
  - New `compute_cmo_from_cmb()` function
  - Added CMO column to `resultado_cmosist.csv` alongside existing PiDemanda
- **Flow dual extraction infrastructure**:
  - `FlowCutMapping` struct to track flow constraint row indices
  - `extract_flow_duals()` function for extracting `λ_L` from LP solution
  - Flow cut naming functions in `labdessem-model`
- **Documentation**: New `docs/design/cmb-cmo-formulation.md` with mathematical formulation

### Changed
- `IterativeSimulationResult` now includes `cmb_records`, `cmo_values`, and `flow_cut_mapping` fields
- `write_cmosist_csv()` accepts optional CMO values parameter
- When `rede=0`: CMO equals PiDemanda, no CMB computation
- When `rede=1`: Full CMB/CMO computation with network congestion component

## [0.8.0] - 2026-10-19

### Removed
- **BREAKING**: Removed deficit variable and `deficit_cost_per_mwh` field from the model
  - Demand balance constraints are now strict equalities: supply must exactly meet demand
  - If the system cannot meet demand, the optimization problem becomes infeasible (no load shedding)
  - Removed `deficit` field from `Variables` struct
  - Removed `deficit` offset from `ColumnOffsets` struct
  - Removed deficit cost loop from `Objective::for_system`
  - Removed deficit term from demand balance constraint
  - Removed `deficit_name()` function from `names.rs`
  - Removed `deficit_cost_per_mwh` field from `Submarket` entity
  - Removed `CustoDeficit` column from `OPER_SBM.csv` format
  - Removed `deficit()` method from `SolutionAccessor`

## [0.7.1] - 2026-10-19

### Fixed
- Fixed hydro volume offset calculation: use `(horizon + 1)` instead of `horizon` for volume variables that exist from t=0 to t=horizon
- Fixed conditional variable allocation in `Indexing`: future_cost, thermal commitment, and hydro commitment offsets now correctly account for disabled features (FCF=0, UCT=0, UCH=0)
- Fixed dual extraction in LP-CALC-CMO: commitment variables now use `Continuous` domain with fixed values instead of `Binary`, allowing proper LP dual extraction
- Fixed case-sensitive infeasibility detection in HiGHS backend: error messages are now compared case-insensitively ("Infeasible" vs "infeasible")

### Added
- Regression test `column_offsets_match_variable_counts` to verify offset consistency with actual variable counts
- Flags `future_cost_enabled`, `thermal_unit_commitment_enabled`, `hydro_unit_commitment_enabled` to `Indexing` struct

## [0.7.0] - 2026-10-19

### Changed
- **BREAKING**: Refactored `labdessem-solver` with clean trait-based architecture
- New `Solver` trait with `SolverBackend` abstraction for multiple solver support
- Extracted core types (`SparseColumn`, `SparseRow`, `Bound`, `Objective`) to dedicated modules
- New `SolutionAccessor` for unified access to primal/dual values
- HiGHS backend implementation with proper separation of concerns
- `Indexing` now includes `ColumnOffsets` and `ConstraintOffsets` for direct solution access
- Removed legacy solver functions from `labdessem-simulation`

### Added
- Integration tests for solver interface (`solve_simple_bounded_lp`, `statistics_accumulate_across_solves`)
- `SolverStatistics` for tracking solve time and iteration counts
- `BasisStatus` enum for LP basis information
- `SolverError` with structured error variants

## [0.6.3] - 2026-10-15

### Fixed
- Fixed period index calculation in `injections_by_period` that caused iterative network cut loop (`opcao_execucao=1`) to stall with error "LP flow-cut loop stalled because the same network violations persisted without generating new cuts"
- Flow calculations now correctly use generation values after the first LP iteration

## [0.6.2] - 2026-10-15

### Fixed
- Mode 1 now correctly skips LP-FIXED loop when MILP has no flow violations
- All output results now come from LP-CALC-CMO stage for economic consistency

### Changed
- Removed `final_summary` from `IterativeSimulationResult` (internal API change)
- Mode 1 without network no longer runs unnecessary LP-FIXED stage

## [0.6.1] - 2026-10-15

### Fixed
- Mode 2 (Single MILP) now includes LP-CALC-CMO stage for valid dual extraction
- Mode 1 LP-CALC-CMO now uses all flow constraints instead of only accumulated cuts

## [0.6.0] - 2026-10-14

### Added
- JSON Schema for `study_config.json` with IDE autocompletion support
- `test_support` module in `labdessem-core` for reusable test fixtures
- Design documentation in `docs/design/` covering architecture decisions
- Workspace-wide lints configuration with clippy pedantic rules

### Changed
- **BREAKING**: CLI now uses subcommand pattern: `labdessem run [CASE_DIR]`
- **BREAKING**: `study_config.json` must be placed inside each case directory
- **BREAKING**: Removed `case_path` field from `study_config.json`
- Binary is now named `labdessem` (configured via `[[bin]]` in Cargo.toml)
- CLI defaults to current directory when case path is omitted
- Refactored `system/mod.rs` from 861 to 31 lines (index-only pattern)
- Refactored `constraints/mod.rs` from 253 to 39 lines (index-only pattern)
- Major architecture improvements: modularized readers, writers, constraints
- Extracted `StudyHorizon` and `OperationalLimit` to dedicated modules

### Removed
- `labdessem-common` crate (was unused)
- `case_path` configuration field

### Fixed
- Improved error message when configuration file is missing
- Resolved all clippy warnings (72 → 0)

## [0.5.0] - 2026-10-13

### Added
- Future cost function (FCF) support with Benders cuts read from binary file
- `FCF` flag in study configuration to enable future cost constraints
- `future_cost` variable and `future_cost_cut` constraints in the model
- `resultado_fcf_eco.csv` output with cut coefficients per hydro plant
- Project logo and branding assets
- Centralized workspace versioning in `Cargo.toml`

### Changed
- Updated README with current model scope, execution modes, and outputs
- Refined website layout and landing page experience
- Updated example cases with new input datasets

## [0.4.0] - 2026-04-30

### Added
- `FPHA` execution flag to switch between hydro generation formulations
- Productivity-based hydro generation (`FPHA=0`) as alternative to FPHA cuts

### Removed
- Hydro ramp trajectories (startup/shutdown)
- Hydro startup and shutdown costs
- Hydro minimum up/down time logic

### Changed
- Simplified hydro unit commitment to on/off status only

## [0.3.0] - 2026-04-24

### Added
- Variable-duration periods with internal half-hour discretization
- Pumping plants (elevatórias) with electrical consumption and hydraulic balance
- Generic renewable plants with unified output file
- LP-CALC-CMO stage for marginal cost extraction
- Execution mode control via `opcao_execucao` flag
- Hydro water withdrawals for consumptive uses
- Hydro diversion flows between plants
- FPHA (linearized hydro production function) constraints
- Unit commitment flags (`UCT`, `UCH`) in study configuration
- Thermal ramp input file (`CAD_RAMPAS_TERMICAS.csv`)
- SIN case without network (`caso_sin_sem_rede`)
- Documentation website with methodology and architecture pages

### Changed
- Expanded hydro units with group-level structure
- Improved iterative process reporting
- Updated study cases for new features

## [0.2.0] - 2026-04-09

### Added
- Documentation site assets
- README with project details

### Changed
- PTDF computation now uses explicit network matrix construction
- Updated thermal input format with new columns
- Revised study configuration schema

### Fixed
- Period duration now uses `f64` for fractional hours

## [0.1.0] - 2026-04-07

### Added
- Initial release with core hydrothermal dispatch model
- Thermal unit commitment with startup/shutdown trajectories
- Minimum up/down time constraints for thermal units
- Hydro scheduling with cascade topology
- Submarket demand balance with deficit variables
- Directed interchange limits between submarkets
- DC network flow with iterative cut addition
- Two execution strategies: iterative (LP→MILP→LP-FIXED) and single MILP
- Operational limit constraints with slack variables
- CSV output files for all result categories
- 7-crate workspace architecture:
  - `labdessem-cli`: command-line interface
  - `labdessem-core`: domain entities and validation
  - `labdessem-io`: CSV/JSON input parsing
  - `labdessem-model`: variables, constraints, objective
  - `labdessem-solver`: HiGHS integration via good_lp
  - `labdessem-simulation`: solve orchestration and output
  - `labdessem-common`: shared utilities
- Example cases: 3Barras, 24Barras

[Unreleased]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.12.0...HEAD
[0.12.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.6.3...v0.7.0
[0.6.3]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/carlosribeiro06/labdessem_rust/releases/tag/v0.1.0
