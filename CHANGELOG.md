# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[0.5.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/carlosribeiro06/labdessem_rust/releases/tag/v0.1.0
