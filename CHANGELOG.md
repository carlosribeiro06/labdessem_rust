# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.13.0] - 2026-11-27

### Added
- Model audit report `docs/auditoria/2026-11-05-auditoria-modelo.md` (in Portuguese); the audit ids cited below (`FP-xx`, `HI-xx`, `EM-xx`, `NU-xx`) are its findings
- FPHA decision records ADR-025 to ADR-034 in `docs/auditoria/fpha-decisions/`, and the FPHA baseline and post-fix evidence in `docs/auditoria/fpha-baseline-reproduction.md`

### Changed
- **BREAKING**: The `labdessem-fpha` crate is removed
  - Before: the FPHA fit, the plant geometry and the max swallowing formula were the public API of the crate `labdessem-fpha`, which built the vendored qhull
  - Now the fit is `labdessem_model::production`, the shared geometry is `labdessem_model::hydro_geometry` and the formula is `labdessem_model::swallowing`; the qhull binding is crate-private (`labdessem_model::hull` is `pub(crate)`); no run changes: every result file and objective is bit-identical (golden capture `994158e`), and only the stderr texts named in the entries below change
  - Now `labdessem-model` compiles the vendored qhull C sources (`links = "qhull_r"`); building it needs a C compiler and the `qhull` submodule at `crates/labdessem-model/vendor/qhull`, which `git submodule update --init --recursive` fills
  - To migrate the fit, replace `labdessem_fpha::<name>` with `labdessem_model::production::<name>` for `FitPlantParams`, `FittingBounds`, `FphaFitDeviation`, `FphaFittingError`, `FphaPlantConfig`, `FphaPlantResult`, `GridParams`, `PlaneReductionConfig`, `ProductionFunction`, `RawPlane`, `build_grid`, `compute_alpha_fpha`, `fit_gamma_s_for_planes`, `fit_hull_planes`, `fit_plant` and `reduce_planes`, and replace `labdessem_fpha::config::<name>` with `labdessem_model::production::<name>` for `VolumeWindow`, `DEFAULT_MAX_PLANES`, `DEFAULT_VOLUME_POINTS`, `DEFAULT_FLOW_POINTS` and `DEFAULT_ANGLE_TOLERANCE_DEG`
  - To migrate the plant geometry, replace `labdessem_fpha::<name>` with `labdessem_model::hydro_geometry::<name>` for `ForebayTable`, `VhaRow` and `TailraceModel`
  - To migrate the max swallowing formula, replace `labdessem_fpha::calculate_max_swallowing` with `labdessem_model::swallowing::calculate_max_swallowing`
  - To migrate, stop calling `labdessem_fpha::<name>` for `build_cloud`, `compute_fit_deviation`, `is_origin_plane`, `resolve_fitting_bounds`, `resolve_s_max`, `scale_plane_affine`, `scale_planes` and `validate_fitted_planes` (no longer public: they are crate-internal in `labdessem_model::production` and `fit_plant` uses them) and for the qhull binding `convex_hull_3d`, `Hyperplane3d` and `HullError` (no longer public: private to `labdessem-model`)
  - To migrate, remove the uses of the names that are removed without replacement (RD-15; ADR-034 for `PlaneReductionConfig::Distance`): `FphaMode`, `fit_plants`, `envelope_max`, `FphaDeviationPoint`, `collect_fit_deviation_points`, `forebay_from_vha` and `tailrace_from_data` (module `from_io`), `TailraceSource` (see the `TailraceSource` entry below), `TailraceModel::Constant`, `VhaRow::area_km2`, the `ProductionFunction` methods `net_head`, `with_max_generation_mw`, `max_generation_mw`, `hydro_name` and `forebay`, the `fitting::rng` module and the `fpha_fit` example
  - To migrate `ProductionFunction::new`, pass `(forebay: ForebayTable, tailrace: TailraceModel, hydraulic_losses_m: f64, efficiency: f64, max_turbined_m3s: f64, max_generation_mw: f64)`; it used to take a `TailraceSource`, two `Option<f64>` (loss and efficiency, defaulting to 0 m and 1.0) and a trailing `hydro_name: String`, and it now applies no default
  - To migrate the changed shapes of the fit types: `FitPlantParams` and `FphaPlantResult` have `id: HydroPlantId` and `name: String` instead of `codigo: i32` and `nome: String`; `FitPlantParams` has `tailrace: TailraceModel`, `hydraulic_loss_m: f64` and `efficiency: f64` instead of three `Option` fields; `FphaPlantConfig` has no `mode` field and no `Default` (build it with `FphaPlantConfig::computed`); `PlaneReductionConfig` is the struct `PlaneReductionConfig { tolerance_deg }` instead of the enum with `Angle` and `Distance`, and `reduce_planes(planes, config)` no longer takes `pf`, `bounds`, `hydro_id` and `entry_level_bits`; `FphaFitDeviation::exceeds_warn_threshold` is removed
  - To migrate the changed shapes of the geometry and the errors: `ForebayTable::new` returns `labdessem_model::hydro_geometry::GeometryError`, and `FphaFittingError` has the variant `Geometry(GeometryError)` instead of `InsufficientPoints` and `DuplicateVolume` and no longer has `DegenerateProductionCloud`, `PlaneValidationFailed` and `IoError`; `SimulationError::Geometry` holds a `GeometryError` instead of an `FphaFittingError`; `TailraceModel` is the struct `TailraceModel { coefficients }`, so a constant level is `coefficients: vec![level_m]`; `ForebayTable` has no `hydro_name` accessor
  - To migrate an existing clone before pulling this change, run:

    ```bash
    git submodule deinit -f crates/labdessem-fpha/vendor/qhull
    git pull
    git submodule update --init --recursive
    rm -rf "$(git rev-parse --git-path modules/crates/labdessem-fpha)"
    rmdir "$(git rev-parse --git-path modules/crates)"
    ```
  - To migrate an existing clone that already pulled this change, run:

    ```bash
    git submodule update --init --recursive
    git config --remove-section submodule.crates/labdessem-fpha/vendor/qhull
    rm -rf crates/labdessem-fpha/vendor && rmdir crates/labdessem-fpha
    rm -rf "$(git rev-parse --git-path modules/crates/labdessem-fpha)"
    rmdir "$(git rev-parse --git-path modules/crates)"
    ```

    If `rmdir` fails, the directory holds files of your own. Inspect it before deleting it.
- **BREAKING**: Rust identifiers say max swallowing instead of EngMax
  - Before: the Rust identifiers of the dynamic maximum swallowing constraint were named `EngMax*` and `engmax*`, and the stderr texts said `EngMax`
  - Now the identifiers say `MaxSwallowing*` and `max_swallowing*`: `EngMaxBounds` -> `MaxSwallowingBounds` in `labdessem-core`, with its module `engmax_bounds` -> `max_swallowing_bounds`; `ConstraintSet::engmax` -> `ConstraintSet::max_swallowing`; `extend_with_engmax` -> `extend_with_max_swallowing`; `ModelError::MissingEngMaxBound` -> `ModelError::MissingMaxSwallowingBound`; `build_engmax_constraints` -> `build_max_swallowing_constraints`; `StudyConfig::engmax` -> `StudyConfig::max_swallowing`, with the serde key still `EngMax`; `EngMaxDiagnostic` -> `MaxSwallowingDiagnostic`; `EngMaxOutcome` -> `MaxSwallowingOutcome`; `IterativeSimulationResult::engmax` -> `IterativeSimulationResult::max_swallowing`; `run_simulation_with_engmax` -> `run_simulation_with_max_swallowing`; `SimulationError::EngMaxInfeasible` -> `SimulationError::MaxSwallowingInfeasible`
  - Now three stderr texts change, old -> new: the Summary line `  EngMax:      <n> plant-periods above the recomputed bound (<k> above a zero bound)` -> `  Swallowing:  <n> plant-periods above the recomputed max swallowing bound (<k> above a zero bound)`; the `EngMax: ` prefix of the `SimulationError` messages -> `max swallowing: `, and the infeasibility message `EngMax: <stage> #<nn> is infeasible with the EngMax rows and feasible without them; the EngMax-free solution exceeds <rows>` -> `max swallowing: <stage> #<nn> is infeasible with the max swallowing rows and feasible without them; the solution without them exceeds <rows>`; `missing EngMax bound for plant <plant> period <period>` -> `missing max swallowing bound for plant <plant> period <period>`
  - Now the same words change in two more places: the `IoError` of `write_hydro_csv` takes the same `missing max swallowing bound for plant <plant> period <period>` text, and the eight `tracing` messages of `labdessem-simulation` say `max swallowing` instead of `EngMax` (the four WARN ones are on stderr by default), for example `EngMax consistency: the final turbining exceeds the recomputed EngMax` -> `max swallowing consistency: the final turbining exceeds the recomputed max swallowing bound` and `Recalculated EngMax bounds` -> `Recalculated max swallowing bounds`
  - The serialized names are unchanged and keep `EngMax`: the `EngMax` key of `study_config.json` and its values, the `engmax[p=<plant>,t=<period>]` LP rows, the `EngMaxHM3` and `EngMaxM3s` columns of `resultado_hidreletricas.csv` with their preamble, and the banner flag that names the config key; no input deck, result file, LP row name or golden key changes
  - To migrate code, rename each identifier of the map above (`EngMax*` -> `MaxSwallowing*`, `engmax*` -> `max_swallowing*`); the `study_config.json` key stays `EngMax`
  - To migrate scripts that parse stderr, match the Summary label `Swallowing:` instead of `EngMax:`, the prefix `max swallowing: ` instead of `EngMax: ` on the simulation error messages, and the new texts above
- **BREAKING**: The io FPHA API is narrowed
  - Before: `labdessem_io::readers::read_vha_geometry`, `read_tailrace`, `VhaPoint`, `VhaGeometryData` and `TailraceData` were public, `read_fpha_config(case_path)` returned `HashMap<u32, FphaPlantConfig>` with the `labdessem_io::readers::FphaPlantConfig` overrides (`num_planes`, `tolerance`, `min_volume_pct`, `max_volume_pct`), and `write_fpha_computed(case_path, &[ComputedFphaRecord])` took `labdessem_io::output::ComputedFphaRecord` rows
  - Now `read_vha_geometry`, `read_tailrace`, `VhaPoint`, `VhaGeometryData` and `TailraceData` are `pub(crate)`, and `labdessem_io::readers::FphaPlantConfig` and `labdessem_io::output::ComputedFphaRecord` are removed
  - Now `read_fpha_config(case_path)` returns `HashMap<HydroPlantId, FphaConfigOverrides>`, where `labdessem_model::production::FphaConfigOverrides` holds the `Option` fields `max_planes`, `tolerance_deg`, `min_volume_fraction` and `max_volume_fraction`
  - Now `write_fpha_computed(case_path, &System, &[FphaPlantResult])` takes the system and the fit results, writes the same CSV as before, and returns an `IoError` without writing the file when a fit names a plant that is not in the system
  - To migrate, call `read_fpha_config` with the new map type (key `HydroPlantId`, value `FphaConfigOverrides`), pass `&System` and the fit results (`&[FphaPlantResult]`) to `write_fpha_computed` instead of building `ComputedFphaRecord` rows, and read the geometry from the `System` (`HydroPlant::volume_level_table` and `tailrace_coefficients`) instead of `read_vha_geometry` and `read_tailrace`
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
  - Max swallowing is unchanged and keeps its defaults for missing efficiency, loss and tailrace
- **BREAKING**: Precomputed FPHA decks are rejected when `OPER_FPHA.csv` has rows for a hydro plant without generating units
  - Before: a plant without rows in `CAD/CAD_CONJ_UHE.csv` kept its `OPER_FPHA.csv` rows, and the model built FPHA constraints for it with no generation or turbining terms
  - Now the reader fails with `OPER_FPHA.csv has FPHA rows for hydro plant(s) without generating units in CAD_CONJ_UHE.csv: <code> (<name>), ...`, listing every affected plant in ascending code order, and the CLI exits with code 1 before writing any result
  - A plant without generating units and without `OPER_FPHA.csv` rows reads as before; computed mode is unchanged
  - To migrate, delete the `OPER_FPHA.csv` rows of the listed plants or add their generating units to `CAD_CONJ_UHE.csv`
- **BREAKING**: The FPHA geometry readers reject non-finite values and duplicate rows
  - Before: `NaN` and `inf` passed the sign checks of the volume, level and area columns of `CAD/CAD_GEOMETRIA_UHE.csv` and of `tolerance` in `CAD/CAD_FPHA_CONFIG.csv`, `CAD/CAD_TAILRACE.csv` had no value check, a second row for the same plant in `CAD_TAILRACE.csv` or `CAD_FPHA_CONFIG.csv` silently replaced the first, and `CAD_GEOMETRIA_UHE.csv` kept two points of one plant with the same `volume.hm3`
  - Now each reader returns `IoError::InvalidData` with `plant <id> has non-finite <column> <value> at line <n> in <file>`, `plant <id> has duplicate rows at lines <a> and <b> in <file>` or `plant <id> has duplicate volume <v> at lines <a> and <b> in CAD_GEOMETRIA_UHE.csv`; when several plants have a duplicate volume, the error names the lowest plant code
  - `CAD_GEOMETRIA_UHE.csv` and `CAD_TAILRACE.csv` are read for every study, so a bad value fails the study read and the CLI exits with code 1 before writing any result; a bad `CAD_FPHA_CONFIG.csv` fails `compute_fpha`
  - Decks with finite values and one row per plant, such as `examples/24Barras`, read as before
  - To migrate, replace each non-finite cell with a finite value and delete the repeated row or point
- **BREAKING**: Computed FPHA fits with the MM defaults and `CAD/CAD_FPHA_CONFIG.csv` takes effect
  - Before: every computed fit used a 6 x 6 volume-flow grid, and no field of the file changed the fit: `num_planes` was copied into the plane limit, which no production code enforced, and `tolerance`, `min_volume_pct` and `max_volume_pct` were read and dropped
  - Now every computed fit starts from 5 volume points, 5 flow points, at most 10 planes and an angle reduction tolerance of 1.0 degrees, so the planes, `alpha` and `resultado_fpha_computed.csv` of every computed run change, with or without the file
  - A row of `CAD_FPHA_CONFIG.csv` changes only its plant: `num_planes` is a plane limit, `tolerance` the angle tolerance in degrees, and `min_volume_pct` / `max_volume_pct` the fractions of the `[Vmin.hm3, Vmax.hm3]` interval of `CAD_UHE.csv` the fit covers (an absent bound keeps 0 or 1)
  - The plane limit is now enforced: the angle reduction is repeated with a doubled tolerance, always from the original plane set and at most 32 times, until the plant has at most `num_planes` planes; when the limit cannot be reached the fit fails with `hydro <name>: cannot reduce to <n> planes (<k> remain after the last tolerance)` and the plant is listed in `PlantsNotFitted`; the fit deviation grows, and `alpha` is fitted on the reduced planes (see the `alpha` entry below)
  - A window outside the geometry table of its plant fails the fit and the plant is listed in `PlantsNotFitted`; a window configured for a plant with a single-point geometry table is ignored and the CLI prints `Warning: plant <id> (<name>) has a single-point table in CAD/CAD_GEOMETRIA_UHE.csv, so min_volume_pct <a> and max_volume_pct <b> in CAD/CAD_FPHA_CONFIG.csv are ignored`
  - The `CAD_FPHA_CONFIG.csv` reader now returns `IoError::InvalidData` with `plant <id> has num_planes 0 (must be at least 1) at line <n> in CAD_FPHA_CONFIG.csv` or `plant <id> has min_volume_pct <a> not below max_volume_pct <b> at line <n> in CAD_FPHA_CONFIG.csv` (the latter only when both bounds are given), and `compute_fpha` fails with it
  - To migrate, set `num_planes` to at least 1 and `min_volume_pct` below `max_volume_pct` in each row, raise `num_planes` for a plant whose fit reports that the limit cannot be reached, and set `tolerance` where the previous plane count must be kept
- **BREAKING**: FPHA rows read the period-mean volume instead of the end-of-period volume (audit FP-06, ADR-030)
  - Before: the `hydro_fpha` row of a period carried `alpha * gamma_v` on the end-of-period volume only, while the max swallowing head volume and the Libs form of the row use the period mean
  - Now every `hydro_fpha` row, computed and precomputed, bounds generation by the planes at the period-mean volume `(V[t] + V[t+1]) / 2`: `alpha * gamma_v / 2` sits on each of the volume slots `t` and `t + 1`, and in the first period (`t=1`), where slot 0 is the fixed initial volume, the row has no slot-0 term and `alpha * gamma_v / 2 * VolInic` moves to the right-hand side
  - Row names, sense and row count are unchanged, and the plants with `gamma_v` 0 (ITUTINGA and FUNIL-GRANDE in `examples/24Barras`) are unaffected
  - `Varm.mw/hm3` of a precomputed `OPER_FPHA.csv` is now a coefficient on the period-mean volume; the columns of `resultado_fpha_computed.csv` and the fit are unchanged
  - `GeracaoMaxPontoMW` follows the period-mean volume; `VolumeHM3` is still the end-of-period volume
  - Measured on `examples/24Barras` before -> after (R$): `O_c` (computed) 2420590.3195 -> 2420558.7522, `O_alpha` (precomputed, `Fcorrec = alpha`) 2420591.0652 -> 2420559.4979, `O_one` (precomputed, `Fcorrec = 1`) 2374998.0326 -> 2374967.2890; the release `FPHA: computed` run `Objective:` 2420590.3195 -> 2420558.7522
  - Maximum static envelope difference `|E(V_mean) - E(V_end)|` along the solved trajectory, before -> after: CAMARGOS 0, ITUTINGA 0, FUNIL-GRANDE 0, FURNAS 0.011043 -> 0.011038 MW (period 9); the maximum change of `GeracaoMaxPontoMW` between the two runs is 0.011084 MW (FURNAS, period 9) and 0 for the other plants
- **BREAKING**: `TailraceSource` is removed
  - Before: `labdessem_fpha::TailraceSource` was a public enum with the variants `Entity(Option<TailraceModel>)` and `Families`; `Families` was documented as a placeholder for backwater coupling, no code constructed it and it evaluated to a 0 m tailrace level
  - Now the tailrace of a production function is the mandatory polynomial `labdessem_model::hydro_geometry::TailraceModel`; no run changes (computed planes, alpha and objectives are bit-identical); tailrace families stay unmodelled (ADR-032 (a); backwater, piecewise ranges and drowning are documented as limitations)
  - To migrate, pass the `TailraceModel` itself where a `TailraceSource::Entity` held `Some(model)`
- **BREAKING**: Computed FPHA fits `alpha` on 30 evaluation points of the final reduced planes (audit B5, ADR-033)
  - Before: `alpha` was fitted on the fit-grid nodes over the raw hull planes, before the gamma_S secant and the plane reduction, and did not depend on `num_planes`
  - Now `alpha` is fitted after the reduction on the planes written to `resultado_fpha_computed.csv`, over 30 evaluation points (6 volume x 5 flow over the fit window and `[0, Q_max]`, 2 x 15 for single-volume plants), so it depends on `num_planes`; the MM default count is 30 and the point distribution is a labdessem choice `[to confirm]`
  - The gamma columns and the plane counts are unchanged; the `alpha` and deviation columns and every computed objective change; the copy rule `Fcorrec = alpha` of precomputed decks is unchanged
  - Measured on `examples/24Barras` before -> after: `alpha` CAMARGOS 0.972834 -> 0.968663, ITUTINGA 1.000000 -> 1.014733, FUNIL-GRANDE 1.000000 -> 1.011552, FURNAS 0.993203 -> 0.989872 (planes 6 / 3 / 3 / 5 before and after); maximum fit deviation of `alpha x planes` (MW) CAMARGOS 2.876295 -> 2.826087, ITUTINGA 0 -> 0.766124, FUNIL-GRANDE 0 -> 2.079277, FURNAS 23.284586 -> 22.245847
  - Objectives (R$): `O_c` (computed) 2420558.7522 -> 2440503.4875, `O_alpha` (precomputed, `Fcorrec = alpha`) 2420559.4979 -> 2440499.3558, `O_one` (precomputed, `Fcorrec = 1`) 2374967.2890 -> 2374967.2890; the release `FPHA: computed` run `Objective:` 2420558.7522 -> 2440503.4875; which plant drives the `O_c` change is `[not measured]`
  - Single-volume plants can get `alpha` above 1 and an envelope above the FPH at the fit-grid nodes: ITUTINGA 1.014733 and FUNIL-GRANDE 1.011552 in `examples/24Barras`
  - The public `labdessem_model::production::compute_alpha_fpha` ignores `n_volume_points` and `n_flow_points` of its bounds and expects the final planes
- **BREAKING**: Public Rust API of the FPHA fit (now `labdessem_model::production`)
  - Before: `FphaPlantConfig::default()` used a 6 x 6 volume-flow grid, `fit_plant` returned `FphaPlantResult::planes` with `gamma_0`, `gamma_v` and `gamma_q` already multiplied by `alpha` and `gamma_s` not, and `FphaFittingError` had no variant for a fit window or an unreachable plane limit
  - Now `FphaFittingError` has the variants `InvalidFitWindow { hydro_name, reason }` and `PlaneLimitUnreachable { hydro_name, max_planes, remaining }`, so an exhaustive `match` must handle them; it is carried by `FitFailureReason::Fitting`
  - Now `FphaPlantConfig` has the public fields `volume_window: production::VolumeWindow` and `window_reference_hm3: Option<(f64, f64)>`, so a struct literal must set them (`FphaPlantConfig::computed` sets the full window); the default grid of a computed fit is 5 x 5 points, from the new constants `production::DEFAULT_VOLUME_POINTS`, `DEFAULT_FLOW_POINTS`, `DEFAULT_MAX_PLANES` and `DEFAULT_ANGLE_TOLERANCE_DEG`
  - Now `fit_plant` returns unscaled `FphaPlantResult::planes` and the fitted `FphaPlantResult::alpha`; the consumer applies `alpha` once to all four coefficients (the model does, as `correction_factor`)
  - To migrate, add the two variants to exhaustive matches, set `volume_window: VolumeWindow::FULL` and `window_reference_hm3: None` in struct literals (or use `FphaPlantConfig::computed`), and apply `alpha` once to all four coefficients wherever the old planes were used directly
- Computed FPHA stderr output follows the model fit results
  - A plant whose VHA table is invalid (empty or with a repeated volume) used to print `plant <id> (<name>): invalid fit parameters: Fitting error: ForebayTable creation failed: <geometry error>` in the `PlantsNotFitted` list; it now prints `plant <id> (<name>): fitting failed: <geometry error>`, where `<geometry error>` is `hydro <name>: insufficient geometry points (<n> < 2)` or `hydro <name>: duplicate volume <v> in geometry table`
  - A plant missing from `inflow_history.parquet` (when the file has rows) used to fail after the `Plant <id> (<name>): fitted <n> planes (α = <alpha>)` lines, and the single-point window `Warning:` lines, of the plants checked before it; now the run fails with the same `hydro <id> (<name>) missing from inflow_history.parquet` message before any such line, because `labdessem_model::production::fit_system` fits every plant before the CLI prints them
- `docs/design/engmax-formulation.md` is renamed `docs/design/max-swallowing-formulation.md`, and `docs/design/fpha-formulation.md` describes the implemented FPHA formulation and its decisions

### Fixed
- `examples/24Barras`: `OPER/OPER_VAZAO.csv` still used the plant codes 1, 2, 4 and 6 after `CAD_UHE.csv` and `CAD_CONJ_UHE.csv` were renumbered 0 to 3, and the hydro reader matches inflows by code only, so the inflow of CAMARGOS went to ITUTINGA and the FUNIL-GRANDE and FURNAS rows were dropped: the model saw 11.65% of the water
  - Only the `Codigo` column changes, and every result of the example changes
  - The reader still accepts an unknown plant code silently (audit HI-01)
- Computed FPHA applied `alpha` twice to `gamma_0`..`gamma_q` and only once to `gamma_s` (audit FP-02, FP-08); the LP now applies `alpha` once to all four coefficients
- A hydro plant with generating units and no fitted FPHA planes was skipped with a warning and generated without water (audit HI-03, FP-01, NU-04); the run now fails naming the plant and the reason
- Computed FPHA silently fitted a plant with a missing tailrace row as a 0 m tailrace level, and a missing `Eficiencia` or `Perda.m` as efficiency 1.0 and loss 0 m, inflating the production function (audit FP-R02); the run now fails naming the plant and every missing input
- Precomputed FPHA rows for a hydro plant without generating units were kept silently and became FPHA constraints with no generation term, hiding an inconsistent deck (audit FP-07); the reader now rejects the deck naming every such plant
- The FPHA geometry readers let `NaN` and infinities through (the sign checks are false for `NaN`), sorted the volume table in an undefined order for `NaN`, and silently kept the last of two rows for the same plant, so a `NaN` water level reached the fit or the net-head model (audit EM-04, FP-R01); the readers now reject the value or the duplicate naming the file, plant, column and line
- `CAD/CAD_FPHA_CONFIG.csv` had no effect on the fit: `num_planes` was copied but never enforced, `tolerance`, `min_volume_pct` and `max_volume_pct` were dropped, and the grid was fixed at 6 x 6 (audit FP-05); each field now changes the fit of its plant and the defaults follow the MM

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

[Unreleased]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.13.0...HEAD
[0.13.0]: https://github.com/carlosribeiro06/labdessem_rust/compare/v0.12.0...v0.13.0
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
