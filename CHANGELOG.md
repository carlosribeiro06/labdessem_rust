# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
