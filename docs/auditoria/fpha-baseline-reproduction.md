# FPHA Baseline Reproduction

This report records the state of the workspace before any FPHA correction: the toolchain, the build,
test, lint and format status of commit `4ac43e0` (develop, v0.12.0 plus the model audit report), and
three fast derived-case runs of the `labdessem` CLI. Every later FPHA ticket compares against these
numbers, so a failure or a number change after a ticket is attributable to that ticket and not to the
starting point. Sections 5 to 7a record the pre-fix A1-A3 measurements (ticket-007); section 8
records the post-fix comparison (ticket-017). Nothing was fixed while measuring; the official example
`examples/24Barras` was never modified or executed.

## 1. Environment

| Item | Value |
|---|---|
| Date | 2026-11-18 |
| Commit | `4ac43e025f65698f08329274eb888bcf4767430f` (`4ac43e0`) |
| Branch | `fix/fpha-corrections` (from `develop` at `4ac43e0`) |
| `rustc -V` | `rustc 1.100.0 (0ef1bc386 2026-11-09)` |
| `cargo -V` | `cargo 1.100.0 (3acc79c0c 2026-11-05)` |
| `cargo clippy -V` | `clippy 0.1.100 (0ef1bc3860 2026-11-09)` |
| `rustfmt -V` | `rustfmt 1.11.0-stable (0ef1bc3860 2026-11-09)` |
| `uname -r` | `6.18.33.2-microsoft-standard-WSL2` |
| qhull submodule SHA | `613debeaea72ee66626dace9ba1a2eff11b5d37d` (`crates/labdessem-fpha/vendor/qhull`, 2020.2) |
| gcc / g++ | `15.2.0` (Ubuntu 15.2.0-16ubuntu1) |
| cmake / make | `4.2.3` / `4.4.1` |
| clang | `Ubuntu clang version 21.1.8 (6ubuntu1)` |
| libclang-dev | `1:21.1.6-71` (`/usr/lib/llvm-21/lib/libclang-21.so`) |
| CPU threads (`nproc`) | 14 |

Notes:

- `git submodule status crates/labdessem-fpha/vendor/qhull` prints the SHA above with a leading space
  (initialized), not a leading `-`.
- No `rust-toolchain*` file exists at the repository root; the toolchain is the rustup default.
- `LIBCLANG_PATH` was unset for every command below: `bindgen` (via `highs-sys`) found libclang on its
  own. An earlier attempt failed at that step because libclang was not installed yet.

## 2. Build and test baseline

All commands ran from the repository root, in this order, with
`. "$HOME/.cargo/env"` sourced first and no file modified.

```bash
cargo build --workspace --release
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

**Exit codes**

- `cargo build --workspace --release`: exit code 0. Cargo reported 11.45 s; an earlier attempt had
  already compiled most dependencies, so this is not a cold-build time.
- `cargo build --workspace`: exit code 0 (191 s wall time, includes compiling the debug dependencies).
- `cargo test --workspace`: exit code 0 (180 s wall time, includes compiling the test binaries).
- `cargo clippy --workspace --all-targets -- -D warnings`: exit code 101.
- `cargo fmt --all -- --check`: exit code 1 (read-only; no file was rewritten).

**Test results** (`cargo test --workspace`, 22 test binaries, no failures):

| Test binary | Passed | Failed | Ignored |
|---|---|---|---|
| labdessem-cli: unittests `src/main.rs` | 24 | 0 | 0 |
| labdessem-cli: `tests/fpha_io_integration.rs` | 5 | 0 | 0 |
| labdessem-cli: `tests/integration.rs` | 0 | 0 | 1 |
| labdessem-core: unittests `src/lib.rs` | 34 | 0 | 0 |
| labdessem-fpha: unittests `src/lib.rs` | 87 | 0 | 0 |
| labdessem-fpha: `tests/integration.rs` | 9 | 0 | 0 |
| labdessem-fpha: `tests/regression.rs` | 7 | 0 | 0 |
| labdessem-io: unittests `src/lib.rs` | 63 | 0 | 0 |
| labdessem-model: unittests `src/lib.rs` | 35 | 0 | 0 |
| labdessem-network: unittests `src/lib.rs` | 47 | 0 | 0 |
| labdessem-simulation: unittests `src/lib.rs` | 58 | 0 | 0 |
| labdessem-solver: unittests `src/lib.rs` | 56 | 0 | 0 |
| labdessem-solver: `tests/integration.rs` | 10 | 0 | 0 |
| labdessem-uc: unittests `src/lib.rs` | 43 | 0 | 0 |
| labdessem-core: doc-tests | 0 | 0 | 1 |
| labdessem-fpha: doc-tests | 0 | 0 | 0 |
| labdessem-io: doc-tests | 0 | 0 | 0 |
| labdessem-model: doc-tests | 0 | 0 | 0 |
| labdessem-network: doc-tests | 0 | 0 | 0 |
| labdessem-simulation: doc-tests | 0 | 0 | 0 |
| labdessem-solver: doc-tests | 0 | 0 | 0 |
| labdessem-uc: doc-tests | 0 | 0 | 0 |
| **Total** | **478** | **0** | **2** |

The two ignored tests are `hydro_output_matches_reference` in `labdessem-cli` `tests/integration.rs`
(`ignored, requires reference file`) and the `SystemBuilder` doc-test at
`crates/labdessem-core/src/system/builder.rs` line 16.

**Clippy** (`cargo clippy --workspace --all-targets -- -D warnings`, exit code 101)

With `-D warnings` the command stops at the first failing unit and no dependent crate is linted.
The failing unit is the `labdessem-fpha` build script: three `clippy::manual_assert` errors at
`crates/labdessem-fpha/build.rs` lines 37, 53 and 66. Re-running the same command with `--keep-going`
(still exit code 101) additionally reports one `clippy::module_inception` error at
`crates/labdessem-core/src/system/mod.rs:10` (the `labdessem-core` lib and lib-test units each emit
it; counted once). The other seven crates are not reached by either run, because Cargo does not lint
the dependents of a failed unit.

To obtain a count per crate, `cargo clippy --workspace --all-targets` was run once more without
`-D warnings` (exit code 0). Both runs use the same lints; `-D warnings` only promotes each warning
to an error, so the table below is the number of diagnostics the denied command would raise once its
blocking errors are fixed. Counts are distinct diagnostics over all targets of the crate (lib, lib
test, bin, integration tests, examples, build script); a diagnostic repeated by the lib and lib-test
units is counted once, matching Cargo's `(N duplicates)` accounting.

| Crate | Warnings |
|---|---|
| labdessem-cli | 13 |
| labdessem-core | 1 |
| labdessem-fpha | 36 |
| labdessem-io | 17 |
| labdessem-model | 5 |
| labdessem-network | 35 |
| labdessem-simulation | 2 |
| labdessem-solver | 55 |
| labdessem-uc | 11 |
| **Total** | **175** |

Dominant lints (distinct diagnostics): `clippy::unwrap_used` 40, `clippy::assert_is_empty` 34,
`clippy::expect_used` 33, `clippy::uninlined_format_args` 11, `clippy::cast_possible_wrap` 9 and
`clippy::collapsible_if` 8; the other 40 are spread over 18 lints. Of the 74 `unwrap_used`,
`expect_used` and `panic` diagnostics, 71 sit in test code (an integration test file, or after the
first `#[cfg(test)]` of the file; the workspace lints in `Cargo.toml` are not relaxed for test
builds) and 3 sit in production code: `crates/labdessem-solver/src/backends/highs.rs` lines 262 and
290 (`expect_used`) and `crates/labdessem-fpha/src/fitting/secant.rs` line 45 (`unwrap_used`).

Reached by the `-D warnings` command itself, `--keep-going` included:

| Crate | Errors under `-D warnings` |
|---|---|
| labdessem-core | 1 |
| labdessem-fpha (build script) | 3 |

**Manifest warnings** (not clippy diagnostics; printed by Cargo itself, new lint
`cargo::unused_dependencies`, set to `warn` by default):

| Manifest | Unused dependency | Printed by |
|---|---|---|
| `crates/labdessem-uc/Cargo.toml:11` | `labdessem-core` | both builds, `cargo test`, `cargo clippy` without `-D warnings` |
| `crates/labdessem-io/Cargo.toml:12` | `labdessem-solver` | both builds, `cargo test` (not in any clippy run) |

Neither the builds nor `cargo test` printed any rustc warning other than these two manifest warnings.

**Format** (`cargo fmt --all -- --check`, exit code 1)

The starting tree is not rustfmt-clean under rustfmt 1.11.0 with edition 2024: the check reports 127
hunks (`cargo fmt --all -- --check 2>&1 | grep -c '^Diff in'` prints 127) in 30 files, all under
`crates/` (`labdessem-fpha` 18, `labdessem-io` 7, `labdessem-cli` 3, `labdessem-core` 2). By a
rough classification of the hunks, about 12 are import ordering (`use std::io::{stderr, IsTerminal}`
against `{IsTerminal, stderr}`) and about 115 re-wrap long lines (long `assert!` calls, string
literals). Running `cargo fmt --all` rewrites those 30 files, so later tickets must not run it
blindly: use `cargo fmt --all -- --check` and format only the files a ticket changes.

## 3. Derived case configurations

All three cases use the same inputs: copies of `examples/24Barras/CAD` and `examples/24Barras/OPER`
plus a new `study_config.json`. The CLI writes its outputs into `RESULTADOS/` inside the case
directory (including `resultado_fpha_computed.csv` in `computed` mode), so it only ever runs on a
temporary copy outside the repository, never inside `examples/24Barras`.

Fast configuration (`study_config.json`, written verbatim, no `$schema` key):

```json
{
  "opcao_execucao": 2,
  "rede": 0,
  "UCT": 0,
  "UCH": 0,
  "TON_Residual": 0,
  "FPHA": "computed",
  "FCF": 1,
  "EngMax": 0
}
```

`EngMax` 0 avoids the computed + UCT + EngMax infeasibility the audit recorded as EM-01; `rede` 0
and `UCT` 0 isolate the FPHA rows and shorten the solve. The third case is the same JSON with
`"FPHA": "constant_productivity"`.

Copy procedure, repeated for each case with a fresh directory:

```bash
repo=/path/to/labdessem_rust
d=$(mktemp -d)
cp -r "$repo/examples/24Barras/CAD" "$repo/examples/24Barras/OPER" "$d"/
# write the fast configuration (FPHA set to the case's mode) to "$d"/study_config.json
start=$(date +%s.%N)
"$repo/target/release/labdessem" run "$d" > run.stdout 2> run.stderr   # or target/debug/labdessem
echo "exit code $? wall $(echo "$(date +%s.%N) - $start" | bc -l) s"
grep -E 'Objective:|Processed' run.stderr
```

The objective and the plant counts are printed on stderr, not stdout.

## 4. Fast derived case runs

| Run | Binary | Exit code | Wall time (s) | Objective | Plants |
|---|---|---|---|---|---|
| fast computed | `target/release/labdessem` | 0 | 0.23 | Objective: 2441046.6443 | Processed 4 plants, 0 skipped |
| fast computed (debug) | `target/debug/labdessem` | 0 | 1.48 | Objective: 2441046.6443 | Processed 4 plants, 0 skipped |
| fast constant_productivity | `target/release/labdessem` | 0 | 0.06 | Objective: 1979326.8490 | not printed (see below) |

- Wall time is the whole process (`date +%s.%N` before and after), one measurement per run.
- The release and debug `computed` runs print the same objective to the four decimals shown.
- The `Processed N plants, M skipped` line is printed only when `FPHA` is `computed`; the
  `constant_productivity` run runs no FPHA computation and prints no such line, so no plant counts
  exist for it.
- The debug run took 1.48 s, far below the 120 s threshold, so no risk is recorded for ticket-004.
- Fitted planes and alpha in the `computed` runs (identical in release and debug): CAMARGOS 6 planes,
  alpha 0.9696; ITUTINGA 3 planes, alpha 1.0000; FUNIL-GRANDE 4 planes, alpha 1.0000; FURNAS 6 planes,
  alpha 0.9913.

## 5. A1 alpha application

Measurements of the `computed` mode before any A1 fix, from the tests of tickets 003 (in-process) and
004 (end to end). Commands, from the repository root with `. "$HOME/.cargo/env"` sourced first.
`LOG_DIR` is a directory outside the repository; `--test-threads=1` keeps the multi-line tables from
interleaving; only stderr goes to the log:

```bash
cargo test -p labdessem-cli baseline_a1 -- --nocapture --test-threads=1 2> "$LOG_DIR/a1.log"
cargo test -p labdessem-cli --test fpha_e2e -- --nocapture --test-threads=1 2> "$LOG_DIR/e2e.log"
```

Both commands exit with code 0 (`baseline_a1`: the 2 in-process tests plus the E2E test of the same
name passed; `--test fpha_e2e`: 3 passed).

In-process table, printed by `baseline_a1_lp_envelope_applies_alpha_twice` and
`baseline_a1_reported_deviation_differs_from_lp_envelope`
(`crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs`, source `a1.log`). Every deviation
cell reads `max/mean/rms` of fitted minus exact production function, in MW:

| Code | Plant | alpha | Planes | ratio_min | ratio_max | Reported, fit grid (MW) | LP-based, fit grid (MW) | LP-based, 11 x 11 grid (MW) |
|---|---|---|---|---|---|---|---|---|
| 0 | CAMARGOS | 0.969607 | 6 | 0.969607257 | 0.969607257 | 3.142964/0.953364/1.291594 | 2.753641/1.042563/1.414023 | 4.897389/1.098339/1.456398 |
| 1 | ITUTINGA | 1.000000 | 3 | 1.000000000 | 1.000000000 | 0.102097/0.032232/0.055914 | 0.102097/0.032232/0.055914 | 2.229353/0.262615/0.677984 |
| 2 | FUNIL-GRANDE | 1.000000 | 4 | 1.000000000 | 1.000000000 | 0.000000/0.000000/0.000000 | 0.000000/0.000000/0.000000 | 5.635867/0.595420/1.705151 |
| 3 | FURNAS | 0.991295 | 6 | 0.991295163 | 0.991295163 | 25.360739/9.047101/11.706933 | 21.078021/9.088040/11.678164 | 48.972971/9.431278/12.618084 |

- Each plant is printed twice in `a1.log`, under the tags `baseline_a1[applies_alpha_twice]` and
  `baseline_a1[reported_deviation]`, with identical numbers; one set is copied.
- `ratio_min` and `ratio_max` are the minimum and maximum of `E_LP / intended` over the fit-grid nodes
  with `q > 0` and `intended >= 1 MW`. They equal alpha for every plant by algebra, not as a measured
  spread: the test rebuilds the unscaled planes as `P_k / alpha` and takes `intended = alpha * min_k
  U_k`, so the ratio is alpha by construction of the check (documented on the first test), and the
  test asserts it within 1e-9 at every qualifying node. Both columns repeat alpha and carry no
  spread information.
- "Reported" is the deviation the pipeline returns (`result.deviation`), asserted equal within
  1e-9 MW to the deviation of the returned planes `min_k P_k` on the fit grid. "LP-based" is the
  deviation of the generation cap read from the period-1 `hydro_fpha` rows of the LP model (`s = 0`),
  both against `evaluate_capped`. The 11 x 11 grid is a uniform `(V, Q)` evaluation grid per plant.
- FUNIL-GRANDE shows 0.000000 on the fit grid, consistent with its 12 fit-grid nodes lying on its 4
  planes, so the 11 x 11 column is the informative one for it. ITUTINGA and FUNIL-GRANDE have
  alpha = 1.000000, so the reported and the LP-based fit-grid deviations coincide for them.
  CAMARGOS and FURNAS (alpha different from 1) show a reported maximum different from the LP-based
  one (3.142964 against 2.753641 MW; 25.360739 against 21.078021 MW).

End-to-end table, printed by `baseline_a1_computed_vs_precomputed_copies`
(`crates/labdessem-cli/tests/fpha_e2e.rs`, source `e2e.log`):

| Label | Run | Objective (R$) | Run time (s) |
|---|---|---|---|
| `O_c` | `FPHA: computed` | 2441046.6443 | 1.60 |
| `O_alpha` | `precomputed` with the planes of `resultado_fpha_computed.csv` and `Fcorrec = alpha` | 2441052.2402 | 1.59 |
| `O_one` | `precomputed` with the same planes and `Fcorrec = 1` (the copy the current `resultado_fpha_computed.csv` preamble documents) | 2383746.0919 | 1.53 |

| Gap | Signed difference | Relative gap printed by the test | Test criterion |
|---|---|---|---|
| `O_alpha - O_c` | +5.5959 R$ | 2.292e-6 | at most 1e-5 relative |
| `O_c - O_one` | +57,300.5524 R$ | 2.347e-2 | more than 1e-5 relative |

- Sign convention: the test prints the absolute relative gaps `|O - O_c| / |O_c|`. The signed
  differences above are computed here from the printed objectives. `O_c - O_one` is positive: the
  computed objective is above the `Fcorrec = 1` copy, the same direction as the audit's +3,125.65 R$.
  `O_alpha - O_c` is positive and tiny: the `Fcorrec = alpha` copy matches the computed objective to
  2.292e-6 relative against the 1e-5 tolerance; the residual is consistent with the CSV rounding of
  the planes (`{:.6}` and `{:.9}` coefficients and a `{:.6}` alpha,
  `crates/labdessem-io/src/output/fpha_computed.rs:72`), not separately isolated.
- The `Objective:` line of the CLI is printed with 4 decimals (`crates/labdessem-cli/src/main.rs:188`);
  every objective above carries that precision, a rounding of at most 5e-5 R$, far below the
  5.5959 R$ residual.
- Run times are whole-process wall times of the DEBUG build, from one single-threaded run of
  `--test fpha_e2e`: they are comparable with the debug row of section 4 (1.48 s), not with its
  release rows. The `baseline_a1` filter also ran this E2E test; its objectives were identical and
  its times (1.49, 1.47 and 1.45 s) are not used, `e2e.log` being the single source.
- The fast configuration (`rede` 0, `UCT` 0, `FCF` 1, `EngMax` 0) gives a 2.35 % objective effect
  here, against the audit's 0.07 % (+3,125.65 R$ on about 4.50e6 under the official configuration),
  so only the sign and the order of magnitude of the effect are comparable (section 7a).

## 6. A2 skipped plants

Measurements of the plants skipped by `computed` before any A2 fix, from the tests of tickets 004
(end to end) and 005 (in-process). Commands, same conventions as section 5. The second command runs
the control test, which the `baseline_a2` filter does not match:

```bash
cargo test -p labdessem-cli baseline_a2 -- --nocapture --test-threads=1 2> "$LOG_DIR/a2.log"
cargo test -p labdessem-cli control_a2 -- --nocapture --test-threads=1 2> "$LOG_DIR/ctl_a2.log"
cargo test -p labdessem-cli --test fpha_e2e -- --nocapture --test-threads=1 2> "$LOG_DIR/e2e.log"
```

All three commands exit with code 0 (`baseline_a2`: 2 in-process tests plus the E2E test of the same
name passed; `control_a2`: 1 passed; `--test fpha_e2e`: 3 passed).

In-process counts for the case without the geometry of plant 2, printed by
`baseline_a2_plant_without_geometry_has_no_fpha_rows`
(`crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs`, source `a2.log`):

| Quantity | Value |
|---|---|
| Geometry rows removed (plant 2, FUNIL-GRANDE) | 1 |
| `plants_processed` | 3 |
| `plants_skipped` | 1 |
| Plant 2 FPHA segments | 0 |
| Plant 2 `hydro_fpha` rows | 0 |
| Plant 2 `hydro_generation_turbining_coupling` rows | 0 |

In-process counts for the case without the geometry file `CAD_GEOMETRIA_UHE.csv`, printed by
`baseline_a2_missing_geometry_file_skips_every_plant` (same file, source `a2.log`):

| Quantity | Value |
|---|---|
| `plants_processed` | 0 |
| `plants_skipped` | 4 (equal to `hydro_plants`, 4) |
| `plant_results` | 0 |
| Injected FPHA segments | 0 |
| `hydro_fpha` rows | 0 |
| `hydro_generation_turbining_coupling` rows | 0 |

- The two cases print different warnings. The missing row of plant 2 prints the per-plant
  `Warning: Plant 2 (FUNIL-GRANDE) has no VHA geometry data, skipping`
  (`crates/labdessem-cli/src/fpha_integration.rs:100`) and still injects 6, 3 and 6 segments for
  plants 0, 1 and 3 (`FPHA injection complete: 3 plants updated`). The missing file prints
  `Warning: No VHA geometry data found. No planes will be computed.` from the early return at
  `crates/labdessem-cli/src/fpha_integration.rs:83` and `FPHA injection complete: 0 plants updated`;
  no per-plant warning appears.

Control with the full geometry, printed by `control_a2_full_geometry_links_every_plant` (same file,
source `ctl_a2.log`): `plants_processed` 4, `plants_skipped` 0, horizon 48 periods.

| Plant | FPHA segments | `hydro_fpha` rows | Units | Coupling rows per unit |
|---|---|---|---|---|
| CAMARGOS | 6 | 288 | 2 | 48 |
| ITUTINGA | 3 | 144 | 4 | 48 |
| FUNIL-GRANDE | 4 | 192 | 3 | 48 |
| FURNAS | 6 | 288 | 8 | 48 |

- `hydro_fpha` rows equal segments times 48 periods for every plant; every unit has one coupling row
  per period. The same plant with its geometry row removed has 0 and 0 (table above).

End-to-end counts for the case without the geometry of plant 2, printed by
`baseline_a2_plant_without_geometry_generates_without_water`
(`crates/labdessem-cli/tests/fpha_e2e.rs`, source `e2e.log`). The counted periods are those with
`GeracaoMW > 1` and `TurbinamentoM3s <= 1e-6` among the aggregate result rows of each plant, one per
period:

| Plant | Periods with generation and no turbining | Aggregate periods | Maximum `GeracaoMW` |
|---|---|---|---|
| FUNIL-GRANDE (plant 2, geometry removed) | 48 | 48 | 180.000000 |
| CAMARGOS | 0 | 48 | not printed |
| FURNAS | 0 | 48 | not printed |
| ITUTINGA | 0 | 48 | not printed |

- The run printed `A2 removed 1 row(s) of plant 2 (FUNIL-GRANDE)`, `objective=2430458.1703` and
  `elapsed=1.35 s`; the test asserts exit code 0 and the skip warning. The elapsed time is a
  whole-process DEBUG-build wall time from one single-threaded run, comparable with the debug row of
  section 4. The `baseline_a2` run reprinted the same counts, objective and maximum (elapsed 1.36 s);
  `e2e.log` is the single source.
- Validation message (asserted, not printed by the test): no test prints the `System::validate`
  message. `baseline_a2_plant_without_geometry_has_no_fpha_rows` asserts that validation with FPHA
  enabled fails for the plant without segments and that the message contains the substring
  `at least one FPHA segment` (`crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs`
  lines 155-161: `validation.expect_err(...)`, then `message.contains("at least one FPHA segment")`).
  The message is built in `crates/labdessem-core/src/entities/hydro.rs` lines 311-315 from the format
  string `"hydro plant {:?} must contain at least one FPHA segment"` with `self.id`, for a plant with
  FPHA enabled, with units and with no segment. The full rendered text, including the plant id, was
  not captured. The test documents that production code does not call `System::validate` after the
  injection; a grep for `.validate(` finds no non-test call under `crates/labdessem-cli/src`.

## 7. A3 physical defaults

Measurements of the silent physical-data defaults of `computed` before any A3 fix, from the tests of
ticket 006. Command, same conventions as section 5:

```bash
cargo test -p labdessem-cli baseline_a3 -- --nocapture --test-threads=1 2> "$LOG_DIR/a3.log"
```

The command exits with code 0 (2 tests passed). Each test prints `[info]` lines with the removed rows
and node counts, and one ratio line per affected plant
(`crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs`, source `a3.log`).

Missing tailrace, printed by `baseline_a3_missing_tailrace_row_uses_zero_level` (the single
`CAD_TAILRACE.csv` row of plant 3 removed; `removed_tailrace_rows=1`, `plants_processed=4`,
`plants_skipped=0`). FURNAS is the only affected plant:

| Code | Plant | FPH ratio, mean | FPH ratio, max | LP-envelope ratio, mean | Fit-grid nodes | FPH nodes | LP-envelope nodes |
|---|---|---|---|---|---|---|---|
| 3 | FURNAS | 8.754581 | 9.842334 | 2.319804 | 36 | 30 | 30 |

Missing efficiency and loss, printed by `baseline_a3_missing_efficiency_and_loss_default_silently`
(the `Perda.m` and `Eficiencia` columns of `CAD_UHE.csv` dropped; all four plants affected):

| Code | Plant | FPH ratio, mean | FPH ratio, max | LP-envelope ratio, mean | Fit-grid nodes | FPH nodes | LP-envelope nodes |
|---|---|---|---|---|---|---|---|
| 0 | CAMARGOS | 1.124287 | 1.127187 | 1.123132 | 36 | 30 | 30 |
| 1 | ITUTINGA | 1.159956 | 1.160583 | 1.122542 | 12 | 10 | 10 |
| 2 | FUNIL-GRANDE | 1.099868 | 1.100352 | 1.079797 | 12 | 10 | 10 |
| 3 | FURNAS | 1.100635 | 1.102037 | 1.093203 | 36 | 30 | 30 |

- The FPH ratio is `derived FPH / control FPH`, uncapped (`evaluate` at `s = 0`), over the control
  fit-grid nodes with `q > 0` and control FPH of at least 1 MW (`fph_nodes`; `fit_grid_nodes` is the
  full control fit grid). The LP-envelope ratio is `envelope_derived / envelope_control` at `s = 0`
  over the nodes that also have a finite control envelope of at least 1 MW (`lp_envelope_nodes`).
  The control is the unmodified case.
- Not comparable: for FURNAS without tailrace the LP-envelope ratio (2.319804) is far below the FPH
  ratio (8.754581), because the hull fit samples `evaluate_capped` and the planes saturate near the
  installed capacity, hiding most of the uncapped inflation. The two ratios are informational and
  must not be read against each other.
- Plane counts and alpha printed by the CLI for the control and the derived runs (`a3.log`). Control:
  CAMARGOS 6 planes, alpha 0.9696; ITUTINGA 3, 1.0000; FUNIL-GRANDE 4, 1.0000; FURNAS 6, 0.9913.
  Missing tailrace, FURNAS: 2 planes, alpha 1.0000. Missing efficiency and loss: CAMARGOS 6 planes,
  alpha 0.9747; ITUTINGA 5, 1.0000; FUNIL-GRANDE 4, 1.0000; FURNAS 9, 0.9940.
- Official data in `examples/24Barras/CAD/CAD_UHE.csv`, in the order CAMARGOS, ITUTINGA,
  FUNIL-GRANDE, FURNAS: `Eficiencia` 0.8937, 0.8817, 0.9185, 0.9170 and `Perda.m` 0.095, 0.631,
  0.394, 0.803 m (rounded from 0.893658694, 0.881693809, 0.918478393, 0.91698296 and 0.094999999,
  0.630999982, 0.393999994, 0.802999973).
- Neither case prints a warning about the missing data (`a3.log` holds no warning line):
  `computed_system` returned `Ok` in both cases and no plant was skipped (`plants_skipped=0`).

## 7a. Comparison with the audit

Audit values from `docs/auditoria/2026-11-05-auditoria-modelo.md` (the runs of its section 12.4:
release binary, example configuration with `rede` 1 and `UCT` 1, line 1712) against the values
reproduced in sections 5 to 7 (debug build, fast configuration of section 3). Line numbers refer to
the audit file.

| Quantity | Audit value | Audit source | Reproduced | Notes |
|---|---|---|---|---|
| alpha, CAMARGOS | 0.969607 | lines 879, 1764 | 0.969607 (`ratio_min` = `ratio_max` = 0.969607257) | Difference 2.57e-7, below the 1e-6 flag threshold. The release and debug builds print the same alpha (section 4). |
| alpha, FURNAS | 0.991295 | lines 880, 1765 | 0.991295 (`ratio_min` = `ratio_max` = 0.991295163) | Difference 1.63e-7, below the 1e-6 flag threshold. Same remark on the builds. |
| alpha, ITUTINGA | 1.000000 | lines 881, 1766 (one row, "ITUTINGA, FUNIL") | 1.000000 (3 planes) | Equal. |
| alpha, FUNIL-GRANDE | 1.000000 | lines 881, 1766 (one row, "ITUTINGA, FUNIL") | 1.000000 (4 planes) | Equal. |
| Objective gap, computed minus precomputed with `Fcorrec = 1` | +3,125.65 R$, computed above | lines 901, 1768; the reviewer's earlier +3,141.8 R$ at line 883 | `O_c - O_one` = +57,300.5524 R$ (relative 2.347e-2) | Same sign. Fast configuration against the official one: only sign and order of magnitude are comparable. DISCREPANCY: the audit states no objective for this comparison (see the objective bullet below). |
| FUNIL-GRANDE generation without water | 180 MW with Q = 0 in 48 of 48 periods (control: none) | lines 648, 663, 1748 (control at line 1747) | 48 of 48 aggregate periods with `GeracaoMW > 1` and `TurbinamentoM3s <= 1e-6`; maximum `GeracaoMW` 180.000000; the other three plants 0 of 48 | Equal count. Fast configuration and debug build here against the audit's release binary; the audit's control (no such period with full geometry) is not re-measured by an E2E control here. |
| FURNAS FPH inflation without efficiency and loss | about 9 % | line 937 (FP-R02) | mean FPH ratio 1.100635 (+10.06 %); max FPH ratio 1.102037 (+10.20 %); mean LP-envelope ratio 1.093203 (+9.32 %); factor from efficiency alone 1/0.9170 = 1.0905 (+9.05 %) | The audit gives no metric definition, so no single quantity is picked as the match. All of them are of the same order, +9 % to +10 %. |

- Alpha: no reproduced alpha differs from the audit by more than 1e-6, so nothing is flagged for the
  decision tickets. The audit's other columns for FP-02 (the ratio of dispatch output to the printed
  plane, 0.969608 and 0.991296 at lines 879-880, 0.969606 and 0.991296 at lines 1764-1765) are a
  different metric and are not reproduced here; the in-process `E_LP / intended` of section 5 equals
  alpha by the algebra stated there.
- Objective attribution: the audit's FP-02 comparison (lines 883, 901 and 1768) states a gap of
  +3,125.65 R$ (and +3,141.8 R$ in the reviewer's earlier measurement) and no objective of its own.
  The objective 4,502,332.7276 belongs to the FP-05 experiment (lines 920 and 1770: three versions
  of `CAD_FPHA_CONFIG` give the same objective), not to FP-02. The audit's base-case objective is
  4,497,639.84 R$ (line 1712: "O objetivo do caso base é 4.497.639,84 R$", example configuration,
  constant productivity unless stated). This report therefore does not assign an objective to the
  audit's FP-02 comparison. Related to the 4.50e6 order of magnitude of the audit's objectives, the
  audit's gap is about 0.07 %; the reproduced gap is 2.35 % of `O_c` (2441046.6443) in the fast
  configuration (`rede` 0, `UCT` 0, `FCF` 1, `EngMax` 0).
- FURNAS inflation: the audit's "cerca de 9 %" is in the findings table (FP-R02, line 937), not in a
  measured table. The reproduced quantities come from the case with `Perda.m` and `Eficiencia`
  dropped together (section 7); the audit's wording leaves open whether one or both columns are
  absent. The tailrace case (FURNAS ratios 8.754581 mean and 9.842334 maximum) is a different input
  removal and is not part of the audit value compared here.
- FUNIL-GRANDE: the reproduced run uses the E2E fast configuration and the debug build of section 4;
  the audit's count comes from its release run with the example configuration. The per-period count
  (48 of 48) and the generation level (180 MW) are equal.

## 8. Post-fix comparison

Measured after tickets 011 to 016, at commit `d6a223d62e0aef6663a16eafada5e4491f5121d7` (`d6a223d`,
branch `fix/fpha-corrections`, tree clean), on 2026-11-19. "Pre" is the frozen sections 2 to 5 (commit
`4ac43e0`); every "post" value was produced by a command run for this section, with the logs kept outside
the repository. No code or test was changed while measuring and `examples/24Barras` was neither modified
nor executed: every CLI run used a fresh derived copy built by the procedure of section 3.

Environment: `rustc`, `cargo`, `clippy` and the qhull SHA equal section 1. The tools on `PATH` differ from
section 1: `gcc`/`g++` 13.3.0 (section 1: 15.2.0), `cmake` 3.28.3 (4.2.3), `clang` 18.1.3 (21.1.8) and
`uname -r` `6.18.58.1-microsoft-standard-WSL2` (`6.18.33.2-microsoft-standard-WSL2`). The release build
compiled three workspace crates only (`labdessem-fpha`, `labdessem-simulation`, `labdessem-cli`); whether the
vendored qhull build script re-ran with these compilers was not checked.

**Build, test and clippy**

```bash
. "$HOME/.cargo/env"
cargo build --workspace --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --message-format=json > "$LOG_DIR/clippy.json"
```

The last command is `cargo clippy --workspace --all-targets` without `-D warnings`; the JSON output is
reduced to distinct diagnostics by the key (lint, primary file, primary line, message), the method of
section 2.

| Command | Exit code, pre | Exit code, post | Note |
|---|---|---|---|
| `cargo build --workspace --release` | 0 | 0 | 11.45 s pre, 2.42 s post; neither is a cold-build time |
| `cargo test --workspace` | 0 | 0 | 180 s pre (includes compiling the test binaries), 5.45 s post (test binaries already compiled); not comparable |
| `cargo clippy --workspace --all-targets -- -D warnings` | 101 | 101 | unchanged; same four errors at the same locations (below) |
| `cargo clippy --workspace --all-targets` | 0 | 0 | 175 distinct diagnostics pre and post |

**Test results** (`cargo test --workspace`: 22 test binaries pre, 23 post, no failures):

| Test binary | Passed, pre | Passed, post | Failed, pre | Failed, post | Ignored, pre | Ignored, post |
|---|---|---|---|---|---|---|
| labdessem-cli: unittests `src/main.rs` | 24 | 44 | 0 | 0 | 0 | 0 |
| labdessem-cli: `tests/fpha_e2e.rs` | absent | 4 | absent | 0 | absent | 0 |
| labdessem-cli: `tests/fpha_io_integration.rs` | 5 | 5 | 0 | 0 | 0 | 0 |
| labdessem-cli: `tests/integration.rs` | 0 | 0 | 0 | 0 | 1 | 1 |
| labdessem-core: unittests `src/lib.rs` | 34 | 34 | 0 | 0 | 0 | 0 |
| labdessem-fpha: unittests `src/lib.rs` | 87 | 89 | 0 | 0 | 0 | 0 |
| labdessem-fpha: `tests/integration.rs` | 9 | 9 | 0 | 0 | 0 | 0 |
| labdessem-fpha: `tests/regression.rs` | 7 | 7 | 0 | 0 | 0 | 0 |
| labdessem-io: unittests `src/lib.rs` | 63 | 63 | 0 | 0 | 0 | 0 |
| labdessem-model: unittests `src/lib.rs` | 35 | 35 | 0 | 0 | 0 | 0 |
| labdessem-network: unittests `src/lib.rs` | 47 | 47 | 0 | 0 | 0 | 0 |
| labdessem-simulation: unittests `src/lib.rs` | 58 | 58 | 0 | 0 | 0 | 0 |
| labdessem-solver: unittests `src/lib.rs` | 56 | 56 | 0 | 0 | 0 | 0 |
| labdessem-solver: `tests/integration.rs` | 10 | 10 | 0 | 0 | 0 | 0 |
| labdessem-uc: unittests `src/lib.rs` | 43 | 43 | 0 | 0 | 0 | 0 |
| labdessem-core: doc-tests | 0 | 0 | 0 | 0 | 1 | 1 |
| labdessem-fpha, -io, -model, -network, -simulation, -solver, -uc: doc-tests (7 binaries) | 0 | 0 | 0 | 0 | 0 | 0 |
| **Total** | **478** | **504** | **0** | **0** | **2** | **2** |

The two ignored tests are the same as in section 2 (`hydro_output_matches_reference` and the
`SystemBuilder` doc-test).

**Clippy** (`cargo clippy --workspace --all-targets`, distinct diagnostics over all targets)

| Crate | Warnings, pre | Warnings, post |
|---|---|---|
| labdessem-cli | 13 | 13 |
| labdessem-core | 1 | 1 |
| labdessem-fpha | 36 | 36 |
| labdessem-io | 17 | 17 |
| labdessem-model | 5 | 5 |
| labdessem-network | 35 | 35 |
| labdessem-simulation | 2 | 2 |
| labdessem-solver | 55 | 55 |
| labdessem-uc | 11 | 11 |
| **Total** | **175** | **175** |

- The dominant lints are unchanged: `clippy::unwrap_used` 40, `clippy::assert_is_empty` 34,
  `clippy::expect_used` 33, `clippy::uninlined_format_args` 11, `clippy::cast_possible_wrap` 9 and
  `clippy::collapsible_if` 8. Only the counts were compared with section 2; the identity of each of the
  175 diagnostics was not, and line numbers move as files change.
- `-D warnings`, exit code 101 pre and post. The errors it reaches are the same as in section 2: three
  `clippy::manual_assert` at `crates/labdessem-fpha/build.rs` lines 37, 53 and 66, and one
  `clippy::module_inception` at `crates/labdessem-core/src/system/mod.rs:10`. Epic 02 does not fix them, so
  the exit code stays 101 (operator decision, 2026-11-19).
- The two manifest warnings of section 2 (`labdessem-uc` -> `labdessem-core`, `labdessem-io` ->
  `labdessem-solver`) are still printed by the build and by `cargo test`; the clippy run prints the
  `labdessem-uc` one. `cargo fmt --all -- --check` was not re-run for this section.

**A1: alpha, planes and deviations**

Commands (`LOG_DIR` outside the repository; `--test-threads=1` keeps the tables from interleaving; only
stderr goes to the log):

```bash
cargo test -p labdessem-cli --bin labdessem -- lp_envelope_matches_alpha_scaled_planes lp_envelope_deviation_matches_reported_deviation computed_csv_round_trip_reproduces_segments --nocapture --test-threads=1 2> "$LOG_DIR/a1.log"
cargo test -p labdessem-cli --test fpha_e2e -- computed_csv_round_trip_reproduces_objective plant_without_geometry_fails_run missing_physical_data_fails_run --nocapture --test-threads=1 2> "$LOG_DIR/e2e.log"
```

Both exit with code 0 (3 and 3 tests passed; the two failure-path tests of the E2E binary are covered
below). The in-process tests print one line per plant under the tags
`lp_envelope[matches_alpha_scaled_planes]` and `lp_envelope[deviation_matches_reported]` with identical
numbers; one set is copied. Pre values are the table of section 5 (maximum deviation, MW, on the fit grid):

| Code | Plant | alpha, pre | alpha, post | Planes, pre | Planes, post | Reported max, pre | Reported max, post | LP-based max, pre | LP-based max, post |
|---|---|---|---|---|---|---|---|---|---|
| 0 | CAMARGOS | 0.969607 | 0.969607 | 6 | 7 | 3.142964 | 2.970530 | 2.753641 | 2.970530 |
| 1 | ITUTINGA | 1.000000 | 1.000000 | 3 | 3 | 0.102097 | 0.102097 | 0.102097 | 0.102097 |
| 2 | FUNIL-GRANDE | 1.000000 | 1.000000 | 4 | 4 | 0.000000 | 0.000000 | 0.000000 | 0.000000 |
| 3 | FURNAS | 0.991295 | 0.991295 | 6 | 6 | 25.360739 | 25.360739 | 21.078021 | 25.360739 |

Full `max/mean/rms` cells (MW), pre from section 5 and post from `a1.log`:

| Code | Plant | Reported, fit grid, pre | Reported, fit grid, post | LP-based, fit grid, pre | LP-based, fit grid, post | LP-based, 11 x 11, pre | LP-based, 11 x 11, post |
|---|---|---|---|---|---|---|---|
| 0 | CAMARGOS | 3.142964/0.953364/1.291594 | 2.970530/0.941389/1.257590 | 2.753641/1.042563/1.414023 | 2.970530/0.941389/1.257590 | 4.897389/1.098339/1.456398 | 3.619181/1.037368/1.378325 |
| 1 | ITUTINGA | 0.102097/0.032232/0.055914 | 0.102097/0.032232/0.055914 | 0.102097/0.032232/0.055914 | 0.102097/0.032232/0.055914 | 2.229353/0.262615/0.677984 | 2.229353/0.262615/0.677984 |
| 2 | FUNIL-GRANDE | 0.000000/0.000000/0.000000 | 0.000000/0.000000/0.000000 | 0.000000/0.000000/0.000000 | 0.000000/0.000000/0.000000 | 5.635867/0.595420/1.705151 | 5.635867/0.595420/1.705151 |
| 3 | FURNAS | 25.360739/9.047101/11.706933 | 25.360739/9.047101/11.706933 | 21.078021/9.088040/11.678164 | 25.360739/9.047101/11.706933 | 48.972971/9.431278/12.618084 | 39.314637/9.982315/12.851147 |

- The post tests no longer print the `ratio_min` and `ratio_max` columns of section 5 (they equalled alpha by
  construction there), so alpha is compared at the 6 decimals the post tests print. The CLI prints the same
  alpha at 4 decimals in the computed runs (CAMARGOS 0.9696, ITUTINGA 1.0000, FUNIL-GRANDE 1.0000, FURNAS
  0.9913). No alpha differs between pre and post at the printed precision, so the 1e-12 threshold could not
  be tested directly. `crates/labdessem-fpha/src/fitting/` has no diff between `4ac43e0` and `d6a223d`, and
  `fit_plant` still computes alpha from the hull planes before any of the changed stages.
- Post, the reported and the LP-based fit-grid deviations are equal for all four plants (before, they
  differed for CAMARGOS and FURNAS).

Round trip of the computed CSV into `OPER_FPHA.csv` (`computed_csv_round_trip_reproduces_segments`, new; no
pre value), printed per plant: `max_envelope_difference` is the largest LP-envelope difference between the
injected segments and the segments read back from the CSV over the fit grid and three `s` values, and
`max_difference_over_bound` is its ratio to the CSV rounding bound the test derives (the test asserts at most
1):

| Code | Plant | Segments | Max envelope difference (MW) | Max difference over bound |
|---|---|---|---|---|
| 0 | CAMARGOS | 7 | 7.742e-5 | 0.597 |
| 1 | ITUTINGA | 3 | 7.200e-5 | 0.762 |
| 2 | FUNIL-GRANDE | 4 | 2.091e-4 | 0.556 |
| 3 | FURNAS | 6 | 6.387e-4 | 0.453 |

**Objectives**

The end-to-end solves are printed by `computed_csv_round_trip_reproduces_objective` (source `e2e.log`):
`O_c` is `FPHA: computed`, `O_alpha` is `precomputed` with the planes of `resultado_fpha_computed.csv` and
`Fcorrec = alpha`, `O_one` is the same planes with `Fcorrec = 1`. Run times are whole-process wall times of
the DEBUG build, one run each; they are comparable with the run times of section 5 only as order of
magnitude.

| Label | Run | Objective pre (R$) | Objective post (R$) | Change (R$) | Run time pre / post (s) |
|---|---|---|---|---|---|
| `O_c` | `FPHA: computed` | 2441046.6443 | 2383481.5598 | -57,565.0845 | 1.60 / 1.43 |
| `O_alpha` | `precomputed`, `Fcorrec = alpha` | 2441052.2402 | 2383483.1342 | -57,569.1060 | 1.59 / 1.50 |
| `O_one` | `precomputed`, `Fcorrec = 1` | 2383746.0919 | 2327100.3261 | -56,645.7658 | 1.53 / 1.65 |

| Gap | Signed difference, pre | Signed difference, post | Relative gap, pre | Relative gap, post | Test criterion |
|---|---|---|---|---|---|
| round-trip gap (`O_alpha - O_c`) | +5.5959 R$ | +1.5744 R$ | 2.292e-6 | 6.605e-7 | at most 1e-5 relative |
| `O_c - O_one` | +57,300.5524 R$ | +56,381.2337 R$ | 2.347e-2 | 2.365e-2 | more than 1e-5 relative |

Release CLI solves of the fast configuration (section 3 procedure on fresh copies, section 4 table format):

| Run | Binary | Exit code | Wall time (s) | Objective | Plants | Solver or MILP infeasibility |
|---|---|---|---|---|---|---|
| fast computed | `target/release/labdessem` | 0 | 0.15 | Objective: 2383481.5598 | Processed 4 plants, 0 skipped | none |
| fast constant_productivity | `target/release/labdessem` | 0 | 0.05 | Objective: 1979326.8490 | not printed | none |

| Objective, R$ | Pre | Post | Change |
|---|---|---|---|
| computed (release CLI) | 2441046.6443 | 2383481.5598 | -57,565.0845 (-2.358 %) |
| constant_productivity (release CLI) | 1979326.8490 | 1979326.8490 | 0.0000 |

- "Infeasibility": the string `infeasib` (case-insensitive) occurs in no stdout, no stderr and no file under
  `RESULTADOS/` of the two solves; both print `violations = 0` and `Violations:  0`. `EngMax` is 0 in the
  fast configuration, so the computed + UCT + EngMax infeasibility of section 3 is not exercised here.
- The computed CLI objective (release) equals `O_c` of the end-to-end test (debug) to the 4 decimals printed.
- The computed run prints, in order, CAMARGOS 7 planes (alpha 0.9696), ITUTINGA 3 (1.0000), FUNIL-GRANDE 4
  (1.0000) and FURNAS 6 (0.9913), `FPHA computation complete: 4 plants processed, 0 skipped` and
  `FPHA injection complete: 4 plants updated`. `resultado_fpha_computed.csv` has 20 plane rows (7, 3, 4 and 6
  per plant) and its preamble now describes the gamma columns as "sem o fator alpha" and alpha as the factor
  the model applies once.

**A2 and A3: plants that can no longer be fitted**

Both cases use the same copy procedure and fast configuration as section 3 (`"FPHA": "computed"`), run with
the release binary on a fresh copy each. Edits (the files have CRLF line endings, which `grep -v` keeps
byte for byte; the header line is kept):

| Case | File | Rows removed | Lines before | Lines after |
|---|---|---|---|---|
| A2 | `CAD/CAD_GEOMETRIA_UHE.csv` | 1: every row of plant code 2 (FUNIL-GRANDE) | 203 | 202 |
| A3 | `CAD/CAD_TAILRACE.csv` | 1: every row of plant code 3 (FURNAS) | 5 | 4 |

```bash
# A2
grep -v '^2;' "$d/CAD/CAD_GEOMETRIA_UHE.csv" > "$d/g.tmp" && mv "$d/g.tmp" "$d/CAD/CAD_GEOMETRIA_UHE.csv"
# A3
grep -v '^3;' "$d/CAD/CAD_TAILRACE.csv" > "$d/t.tmp" && mv "$d/t.tmp" "$d/CAD/CAD_TAILRACE.csv"
"$repo/target/release/labdessem" run "$d" > run.stdout 2> run.stderr; echo "exit code $?"
```

| Case | Exit code, pre | Exit code, post | First error line, post (stderr) | Detail line, post |
|---|---|---|---|---|
| A2 | 0 (section 6: skip warning, objective 2430458.1703, debug E2E) | exit code 1 | `Error: computed FPHA could not be fitted for 1 hydro plant(s) with generating units:` | `plant 2 (FUNIL-GRANDE): no rows in CAD/CAD_GEOMETRIA_UHE.csv` |
| A3 | not recorded (section 7: in-process `Ok`, no warning, no plant skipped) | exit code 1 | `Error: computed FPHA could not be fitted for 1 hydro plant(s) with generating units:` | `plant 3 (FURNAS): missing inputs: row in CAD/CAD_TAILRACE.csv` |

- The first line of stderr is the blank line that precedes the banner; "first error line" is the first line
  that starts with `Error:`. Before it, A2 prints the fits of plants 0, 1 and 3 (7, 3 and 6 planes) and A3
  those of plants 0, 1 and 2 (7, 3 and 4 planes). The detail line is the next line, indented by two spaces.
- Both runs write nothing to stdout besides the `Lendo arquivo:` lines and create no `RESULTADOS/`
  directory. Wall time 0.01 s each. The A2 per-plant `Warning: Plant 2 (FUNIL-GRANDE) has no VHA geometry
  data, skipping` of section 6 is not printed.

Failure-path tests, run with `--nocapture` (all pass). The two E2E rows come from the `e2e.log` command of
the A1 part above; the two unit rows from:

```bash
cargo test -p labdessem-cli --bin labdessem -- missing_tailrace_row_fails_fitting missing_efficiency_and_loss_fail_fitting --nocapture --test-threads=1 2> "$LOG_DIR/a3_unit.log"
```

| Test | Binary | Result | Printed |
|---|---|---|---|
| `plant_without_geometry_fails_run` | `tests/fpha_e2e.rs` | ok | `A2 removed 1 row(s) of plant 2 (FUNIL-GRANDE) from CAD/CAD_GEOMETRIA_UHE.csv` |
| `missing_physical_data_fails_run` | `tests/fpha_e2e.rs` | ok | `A3 removed 1 row(s) of plant 3 (FURNAS) from CAD/CAD_TAILRACE.csv` |
| `missing_tailrace_row_fails_fitting` | unittests `src/main.rs` | ok | fits of CAMARGOS, ITUTINGA and FUNIL-GRANDE (7, 3, 4 planes), none for FURNAS |
| `missing_efficiency_and_loss_fail_fitting` | unittests `src/main.rs` | ok | `Computing FPHA planes from geometry data...` only; no plant fitted |

The unit tests assert the error and print no message text; the message text above comes from the two
release CLI runs. These failure-path tests replace the `baseline_a2_*` and `baseline_a3_*` tests of
sections 6 and 7; no test name in `cargo test --workspace -- --list` contains `baseline` any more.

**Differences**

Intended by the plan: alpha is applied once (the LP-based deviation equals the reported one, and the
computed objective moves with it), and the A2 and A3 cases stop with exit code 1 and a typed error instead
of skipping the plant or using a default. Every other pre/post change found is listed here; none was
explained by an isolating experiment.

- Plane count: CAMARGOS moves from 6 to 7 planes; ITUTINGA (3), FUNIL-GRANDE (4) and FURNAS (6) are
  unchanged. The injected segments per computed run go from 19 to 20, and by the section 6 rule (segments
  times 48 periods) the `hydro_fpha` rows of CAMARGOS go from 288 to 336. Under `crates/labdessem-fpha/src`
  only `pipeline.rs` differs between `4ac43e0` and `d6a223d`: the gamma_S secant fit and the plane
  reduction now run on the unscaled planes, and the deviation is evaluated on alpha times the planes.
  `crates/labdessem-cli/src/fpha_integration.rs` and `crates/labdessem-io/src/output/fpha_computed.rs` also
  differ (error handling, validation, CSV preamble). The ticket-011 learnings had anticipated CAMARGOS 6 to
  7; no run separated its cause among these changes.
- Deviation of CAMARGOS: the reported maximum moves from 3.142964 to 2.970530 MW (mean 0.953364 to
  0.941389, rms 1.291594 to 1.257590). FURNAS keeps the reported cells of section 5 to the last printed digit
  (25.360739/9.047101/11.706933); ITUTINGA and FUNIL-GRANDE are unchanged.
- LP-based deviation on the 11 x 11 grid: CAMARGOS 4.897389 to 3.619181 MW and FURNAS 48.972971 to
  39.314637 MW (max); ITUTINGA and FUNIL-GRANDE are unchanged. On the fit grid the LP-based value of CAMARGOS
  (2.753641 to 2.970530) and FURNAS (21.078021 to 25.360739) now equal the reported one.
- Objectives: the computed objective moves from 2441046.6443 to 2383481.5598 R$ (-57,565.0845 R$,
  -2.358 %), `O_alpha` by -57,569.1060 R$ and `O_one` by -56,645.7658 R$. The `Fcorrec = 1` copy is not the
  same case as before: its planes are now unscaled, so alpha is applied zero times instead of once. The
  post computed objective lies 264.5321 R$ (1.11e-4 relative) below the pre `O_one` copy, which applied alpha
  once; the CAMARGOS plane count is the other change in the planes, and the contributions were not separated.
  The round-trip gap `O_alpha - O_c` falls from +5.5959 R$ (2.292e-6) to +1.5744 R$ (6.605e-7), and
  `O_c - O_one` goes from 2.347e-2 to 2.365e-2 relative; the post criteria (at most 1e-5, more than 1e-5)
  hold. The constant_productivity objective is unchanged (1979326.8490), as expected for a mode that builds no
  FPHA rows.
- Exit codes and outputs: A2 goes from exit code 0 (objective 2430458.1703 in section 6) to exit code 1 with
  no output directory; A3 has no pre-fix CLI exit code to compare. The two solving cases keep exit code 0 and
  show no infeasibility.
- Test inventory: 22 to 23 test binaries (new `labdessem-cli` `tests/fpha_e2e.rs`, 4 tests) and 478 to 504
  passed (+26: `labdessem-cli` unit tests +20, `fpha_e2e` +4, `labdessem-fpha` unit tests +2); failures 0 and
  ignored 2 before and after. The tests print different tags and columns from sections 5 to 7 (the `baseline_*`
  tags, `ratio_min` and `ratio_max` are gone; `lp_envelope[...]` and `round_trip` lines are new).
- Unchanged: clippy (175 distinct, CLI 13, fpha 36, `-D warnings` exit code 101 with the same four errors),
  the manifest warnings, alpha at the printed precision, the ignored tests.
- Not comparable: wall times are single runs (0.23 to 0.15 s computed release, 0.06 to 0.05 s
  constant_productivity release, debug E2E times in the objective table) and no repeat was made, so no timing
  change is claimed; the compilers and kernel on `PATH` differ from section 1 (Environment above).

## 9. Post-configuration comparison

Measured after tickets 019, 021, 023, 024 and 031, at commit `51751baf453d2b8840c66f0754b48d30c4348e86`
(`51751ba`, branch `fix/fpha-corrections`, `git status --short` empty), on 2026-11-19. "Pre" is section 8 (its
"post" column, commit `d6a223d`); every "post" value was produced by a command run for this section, with the
logs kept outside the repository. Between `d6a223d` and `27a608b` (the Epic 02 boundary commit) the only change
under `crates/` is test code (`crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs`, `git diff
--stat d6a223d..27a608b -- crates`), so section 8 stands as the pre-Epic 03 state. No code or test was changed
while measuring and `examples/24Barras` was neither modified nor executed: every CLI run used a fresh derived
copy built by the procedure of section 3.

Environment: `rustc`, `cargo`, `clippy`, `rustfmt` and the qhull SHA equal section 1; `gcc`/`g++` 13.3.0, `cmake`
3.28.3, `clang` 18.1.3 and `uname -r` `6.18.58.1-microsoft-standard-WSL2` equal the values section 8 lists for its
`PATH` (section 1: 15.2.0, 4.2.3, 21.1.8 and `6.18.33.2-microsoft-standard-WSL2`); `make` is 4.3 (section 1:
4.4.1) and `nproc` 14 (section 1: 14). The release build compiled the nine workspace crates (section 8: three);
the vendored qhull build script was not checked.

**Build, test and clippy**

```bash
. "$HOME/.cargo/env"
cargo build --workspace --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --message-format=json > "$LOG_DIR/clippy.json"
```

The last command is `cargo clippy --workspace --all-targets` without `-D warnings`; the JSON output is reduced to
distinct diagnostics by the key (lint, primary file, primary line, message), the method of section 2 (messages
without a primary span, such as the `N warnings emitted` summaries, are not counted).

| Command | Exit code, pre | Exit code, post | Note |
|---|---|---|---|
| `cargo build --workspace --release` | 0 | 0 | 2.42 s pre, 9.31 s post; the nine workspace crates were recompiled post; neither is a cold-build time |
| `cargo test --workspace` | 0 | 0 | 5.45 s pre (test binaries already compiled), 23.53 s post (the nine workspace crates compiled); not comparable |
| `cargo clippy --workspace --all-targets -- -D warnings` | 101 | 101 | unchanged; same four errors at the same locations (below) |
| `cargo clippy --workspace --all-targets` | 0 | 0 | 175 distinct diagnostics pre, 174 post |

**Test results** (`cargo test --workspace`: 23 test binaries pre and post, no failures):

| Test binary | Passed, pre | Passed, post | Failed, pre | Failed, post | Ignored, pre | Ignored, post |
|---|---|---|---|---|---|---|
| labdessem-cli: unittests `src/main.rs` | 44 | 55 | 0 | 0 | 0 | 0 |
| labdessem-cli: `tests/fpha_e2e.rs` | 4 | 5 | 0 | 0 | 0 | 0 |
| labdessem-cli: `tests/fpha_io_integration.rs` | 5 | 5 | 0 | 0 | 0 | 0 |
| labdessem-cli: `tests/integration.rs` | 0 | 0 | 0 | 0 | 1 | 1 |
| labdessem-core: unittests `src/lib.rs` | 34 | 34 | 0 | 0 | 0 | 0 |
| labdessem-fpha: unittests `src/lib.rs` | 89 | 94 | 0 | 0 | 0 | 0 |
| labdessem-fpha: `tests/integration.rs` | 9 | 9 | 0 | 0 | 0 | 0 |
| labdessem-fpha: `tests/regression.rs` | 7 | 8 | 0 | 0 | 0 | 0 |
| labdessem-io: unittests `src/lib.rs` | 63 | 74 | 0 | 0 | 0 | 0 |
| labdessem-model: unittests `src/lib.rs` | 35 | 35 | 0 | 0 | 0 | 0 |
| labdessem-network: unittests `src/lib.rs` | 47 | 47 | 0 | 0 | 0 | 0 |
| labdessem-simulation: unittests `src/lib.rs` | 58 | 58 | 0 | 0 | 0 | 0 |
| labdessem-solver: unittests `src/lib.rs` | 56 | 56 | 0 | 0 | 0 | 0 |
| labdessem-solver: `tests/integration.rs` | 10 | 10 | 0 | 0 | 0 | 0 |
| labdessem-uc: unittests `src/lib.rs` | 43 | 43 | 0 | 0 | 0 | 0 |
| labdessem-core: doc-tests | 0 | 0 | 0 | 0 | 1 | 1 |
| labdessem-fpha, -io, -model, -network, -simulation, -solver, -uc: doc-tests (7 binaries) | 0 | 0 | 0 | 0 | 0 | 0 |
| **Total** | **504** | **533** | **0** | **0** | **2** | **2** |

The two ignored tests are the same as in section 2.

**Clippy** (`cargo clippy --workspace --all-targets`, distinct diagnostics over all targets)

| Crate | Warnings, pre | Warnings, post |
|---|---|---|
| labdessem-cli | 13 | 12 |
| labdessem-core | 1 | 1 |
| labdessem-fpha | 36 | 36 |
| labdessem-io | 17 | 17 |
| labdessem-model | 5 | 5 |
| labdessem-network | 35 | 35 |
| labdessem-simulation | 2 | 2 |
| labdessem-solver | 55 | 55 |
| labdessem-uc | 11 | 11 |
| **Total** | **175** | **174** |

- The count is below the 175 of section 2, so no new diagnostic is recorded. The dominant lints equal section 8
  except `clippy::collapsible_if`: `clippy::unwrap_used` 40, `clippy::assert_is_empty` 34, `clippy::expect_used`
  33, `clippy::uninlined_format_args` 11, `clippy::cast_possible_wrap` 9, `clippy::collapsible_if` 7 (pre 8).
  Only the counts were compared; the identity of the one diagnostic that left `labdessem-cli` was not
  established, and line numbers move as files change.
- `-D warnings`, exit code 101 pre and post, the same four errors: three `clippy::manual_assert` at
  `crates/labdessem-fpha/build.rs` lines 37, 53 and 66, and one `clippy::module_inception` at
  `crates/labdessem-core/src/system/mod.rs:10`.
- The two manifest warnings of section 2 (`labdessem-uc` -> `labdessem-core`, `labdessem-io` ->
  `labdessem-solver`) are still printed by the release build and by `cargo test`; the clippy run prints the
  `labdessem-uc` one. `cargo fmt --all -- --check` was not re-run for this section.

**A1: alpha, planes and deviations**

Commands (`LOG_DIR` outside the repository; only stderr goes to the log; the three commands are those of section
8, no test was renamed):

```bash
cargo test -p labdessem-cli --bin labdessem -- lp_envelope_matches_alpha_scaled_planes lp_envelope_deviation_matches_reported_deviation computed_csv_round_trip_reproduces_segments --nocapture --test-threads=1 2> "$LOG_DIR/a1.log"
cargo test -p labdessem-cli --test fpha_e2e -- computed_csv_round_trip_reproduces_objective plant_without_geometry_fails_run missing_physical_data_fails_run --nocapture --test-threads=1 2> "$LOG_DIR/e2e.log"
cargo test -p labdessem-cli --bin labdessem -- missing_tailrace_row_fails_fitting missing_efficiency_and_loss_fail_fitting --nocapture --test-threads=1 2> "$LOG_DIR/a3_unit.log"
```

All three exit with code 0 (3, 3 and 2 tests passed). Pre values are the post columns of section 8; alpha is
printed at the 6 decimals of `a1.log`, maximum deviations in MW on the fit grid:

| Code | Plant | alpha, pre | alpha, post | Planes, pre | Planes, post | Reported max, pre | Reported max, post | LP-based max, pre | LP-based max, post |
|---|---|---|---|---|---|---|---|---|---|
| 0 | CAMARGOS | 0.969607 | 0.972834 | 7 | 6 | 2.970530 | 2.876295 | 2.970530 | 2.876295 |
| 1 | ITUTINGA | 1.000000 | 1.000000 | 3 | 3 | 0.102097 | 0.000000 | 0.102097 | 0.000000 |
| 2 | FUNIL-GRANDE | 1.000000 | 1.000000 | 4 | 3 | 0.000000 | 0.000000 | 0.000000 | 0.000000 |
| 3 | FURNAS | 0.991295 | 0.993203 | 6 | 5 | 25.360739 | 23.284586 | 25.360739 | 23.284586 |

Full `max/mean/rms` cells (MW), pre from section 8 and post from `a1.log`:

| Code | Plant | Reported, fit grid, pre | Reported, fit grid, post | LP-based, fit grid, pre | LP-based, fit grid, post | LP-based, 11 x 11, pre | LP-based, 11 x 11, post |
|---|---|---|---|---|---|---|---|
| 0 | CAMARGOS | 2.970530/0.941389/1.257590 | 2.876295/0.880778/1.230593 | 2.970530/0.941389/1.257590 | 2.876295/0.880778/1.230593 | 3.619181/1.037368/1.378325 | 3.933533/1.062275/1.428675 |
| 1 | ITUTINGA | 0.102097/0.032232/0.055914 | 0.000000/0.000000/0.000000 | 0.102097/0.032232/0.055914 | 0.000000/0.000000/0.000000 | 2.229353/0.262615/0.677984 | 2.673366/0.366313/0.851925 |
| 2 | FUNIL-GRANDE | 0.000000/0.000000/0.000000 | 0.000000/0.000000/0.000000 | 0.000000/0.000000/0.000000 | 0.000000/0.000000/0.000000 | 5.635867/0.595420/1.705151 | 6.806018/1.049644/2.196044 |
| 3 | FURNAS | 25.360739/9.047101/11.706933 | 23.284586/7.746639/10.683041 | 25.360739/9.047101/11.706933 | 23.284586/7.746639/10.683041 | 39.314637/9.982315/12.851147 | 43.053387/9.748834/12.874282 |

- The CLI prints alpha at 4 decimals in the computed runs: CAMARGOS 0.9728, ITUTINGA 1.0000, FUNIL-GRANDE 1.0000,
  FURNAS 0.9932. Alpha of CAMARGOS and FURNAS now differs from section 8 at the printed precision (+0.003227 and
  +0.001908); section 8 found no alpha difference between `4ac43e0` and `d6a223d`.
- Post, the reported and the LP-based fit-grid deviations are equal for all four plants, as in section 8.

Round trip of the computed CSV into `OPER_FPHA.csv` (`computed_csv_round_trip_reproduces_segments`, columns as in
section 8; the test asserts `max_difference_over_bound` at most 1):

| Code | Plant | Segments, pre | Segments, post | Max envelope difference (MW), pre | Max envelope difference (MW), post | Max difference over bound, pre | Max difference over bound, post |
|---|---|---|---|---|---|---|---|
| 0 | CAMARGOS | 7 | 6 | 7.742e-5 | 1.100e-4 | 0.597 | 0.848 |
| 1 | ITUTINGA | 3 | 3 | 7.200e-5 | 1.180e-4 | 0.762 | 0.783 |
| 2 | FUNIL-GRANDE | 4 | 3 | 2.091e-4 | 8.400e-5 | 0.556 | 0.248 |
| 3 | FURNAS | 6 | 5 | 6.387e-4 | 9.419e-4 | 0.453 | 0.669 |

**Objectives**

The end-to-end solves are printed by `computed_csv_round_trip_reproduces_objective` (source `e2e.log`), with the
labels of section 8 (`O_c` computed, `O_alpha` precomputed with `Fcorrec = alpha`, `O_one` the same planes with
`Fcorrec = 1`). Run times are whole-process wall times of the DEBUG build, one run each.

| Label | Run | Objective pre (R$) | Objective post (R$) | Change (R$) | Run time pre / post (s) |
|---|---|---|---|---|---|
| `O_c` | `FPHA: computed` | 2383481.5598 | 2420590.3195 | +37,108.7597 (+1.557 %) | 1.43 / 1.57 |
| `O_alpha` | `precomputed`, `Fcorrec = alpha` | 2383483.1342 | 2420591.0652 | +37,107.9310 (+1.557 %) | 1.50 / 1.60 |
| `O_one` | `precomputed`, `Fcorrec = 1` | 2327100.3261 | 2374998.0326 | +47,897.7065 (+2.058 %) | 1.65 / 1.64 |

| Gap | Signed difference, pre | Signed difference, post | Relative gap, pre | Relative gap, post | Test criterion |
|---|---|---|---|---|---|
| round-trip gap (`O_alpha - O_c`) | +1.5744 R$ | +0.7457 R$ | 6.605e-7 | 3.081e-7 | at most 1e-5 relative |
| `O_c - O_one` | +56,381.2337 R$ | +45,592.2869 R$ | 2.365e-2 | 1.884e-2 | more than 1e-5 relative |

Release CLI solves of the fast configuration (section 3 procedure on fresh copies, section 4 table format):

| Run | Binary | Exit code | Wall time (s) | Objective | Plants | Solver or MILP infeasibility |
|---|---|---|---|---|---|---|
| fast computed | `target/release/labdessem` | 0 | 0.15 | Objective: 2420590.3195 | Processed 4 plants, 0 skipped | none |
| fast constant_productivity | `target/release/labdessem` | 0 | 0.05 | Objective: 1979326.8490 | not printed | none |

| Objective, R$ | Pre | Post | Change |
|---|---|---|---|
| computed (release CLI) | 2383481.5598 | 2420590.3195 | +37,108.7597 (+1.557 %) |
| constant_productivity (release CLI) | 1979326.8490 | 1979326.8490 | 0.0000 |

- "Infeasibility": the string `infeasib` (case-insensitive) occurs in no stdout, no stderr and no file under
  `RESULTADOS/` of the two solves; both print `violations = 0` and `Violations:  0`.
- The computed CLI objective (release) equals `O_c` of the end-to-end test (debug) to the 4 decimals printed.
- The computed run prints, in order, CAMARGOS 6 planes (alpha 0.9728), ITUTINGA 3 (1.0000), FUNIL-GRANDE 3
  (1.0000) and FURNAS 5 (0.9932), `FPHA computation complete: 4 plants processed, 0 skipped` and
  `FPHA injection complete: 4 plants updated`, with `Injected 6`, `3`, `3` and `5 FPHA segments`.
  `resultado_fpha_computed.csv` has 17 plane rows (6, 3, 3 and 5 per plant); section 8 had 20.

**A2 and A3: plants that can no longer be fitted**

Same copies, edits and commands as the A2 and A3 part of section 8 (release binary, `"FPHA": "computed"`, fresh
copy each; A2 removes the one row of plant 2 from `CAD/CAD_GEOMETRIA_UHE.csv`, 203 to 202 lines; A3 removes the
one row of plant 3 from `CAD/CAD_TAILRACE.csv`, 5 to 4 lines):

| Case | Exit code, pre | Exit code, post | First error line, post (stderr) | Detail line, post |
|---|---|---|---|---|
| A2 | exit code 1 | exit code 1 | `Error: computed FPHA could not be fitted for 1 hydro plant(s) with generating units:` | `plant 2 (FUNIL-GRANDE): no rows in CAD/CAD_GEOMETRIA_UHE.csv` |
| A3 | exit code 1 | exit code 1 | `Error: computed FPHA could not be fitted for 1 hydro plant(s) with generating units:` | `plant 3 (FURNAS): missing inputs: row in CAD/CAD_TAILRACE.csv` |

Both runs print the fits of the three other plants first (A2: plants 0, 1 and 3 with 6, 3 and 5 planes; A3: plants
0, 1 and 2 with 6, 3 and 3 planes), write no `RESULTADOS/` directory and take 0.01 s. The four failure-path tests
of section 8 pass with the commands above. The two E2E tests print `A3 removed 1 row(s) of plant 3 (FURNAS) from
CAD/CAD_TAILRACE.csv` and `A2 removed 1 row(s) of plant 2 (FUNIL-GRANDE) from CAD/CAD_GEOMETRIA_UHE.csv`;
`missing_tailrace_row_fails_fitting` prints the fits of CAMARGOS, ITUTINGA and FUNIL-GRANDE (6, 3 and 3 planes) and
none for FURNAS; `missing_efficiency_and_loss_fail_fitting` prints only `Computing FPHA planes from geometry
data...`.

**Configuration effects on derived cases**

Each case is a copy of `examples/24Barras` (`CAD` and `OPER`) with the fast configuration of section 3
(`"FPHA": "computed"`) and one `CAD/CAD_FPHA_CONFIG.csv` of header plus one row, written as in the ticket-031
tests `config_tolerance_changes_only_its_plant`, `config_window_changes_only_its_plant` and
`config_plane_limit_caps_its_plant` (`crates/labdessem-cli/src/fpha_integration.rs`). One fresh directory and
one release CLI run per row:

```bash
repo=/path/to/labdessem_rust
d=$(mktemp -d)
cp -r "$repo/examples/24Barras/CAD" "$repo/examples/24Barras/OPER" "$d"/
# write the fast configuration (FPHA computed) to "$d"/study_config.json, as in section 3
printf 'codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct\n%s\n' "$ROW" > "$d/CAD/CAD_FPHA_CONFIG.csv"
"$repo/target/release/labdessem" run "$d" > run.stdout 2> run.stderr; echo "exit code $?"
grep -E 'fitted|Objective:|Processed|Warning|Error' run.stderr
```

`ROW` is the first column below; the control has no `CAD_FPHA_CONFIG.csv` (it is the "fast computed" run of the
objective table). Alpha and the maximum fit-grid deviation of the edited plant come from the
`resultado_fpha_computed.csv` of each run (deviation at the 3 decimals that file prints, on the fit grid of that
run, so it is not comparable across rows with different windows). Change and relative change are against the control
objective 2420590.3195:

| `ROW` | Edited plant | Planes, control -> row | alpha, edited plant | Max deviation, edited plant (MW) | Exit code | Objective (R$) | Change (R$) | Relative |
|---|---|---|---|---|---|---|---|---|
| none (control) | none | CAMARGOS 6, ITUTINGA 3, FUNIL-GRANDE 3, FURNAS 5 | CAMARGOS 0.972834, FURNAS 0.993203 | CAMARGOS 2.876, FURNAS 23.285 | 0 | 2420590.3195 | 0.0000 | 0 |
| `3;;5.0;;` | FURNAS | 5 -> 2 | 0.993203 | 182.958 | 0 | 1979371.7022 | -441,218.6173 | -18.228 % |
| `3;;;0.5;0.6` | FURNAS | 5 -> 2 | 0.999463 | 24.274 | 0 | 1979381.5955 | -441,208.7240 | -18.227 % |
| `0;2;;;` | CAMARGOS | 6 -> 2 | 0.972834 | 20.701 | 0 | 2406551.2572 | -14,039.0623 | -0.580 % |
| `3;2;5.0;0.5;0.6` (audit row) | FURNAS | 5 -> 2 | 0.999463 | 24.274 | 0 | 1979381.5955 | -441,208.7240 | -18.227 % |
| `3;1;;;` (extra, see below) | FURNAS | 5 -> error | not printed | not printed | 1 | no objective | not applicable | not applicable |

- Every run exits 0 and prints `Processed 4 plants, 0 skipped` except the extra row. In every row the other three
  plants keep the control's planes, alpha at 4 decimals (stderr) and alpha and deviations at the CSV precision.
  No run printed a `Warning` line: no row touches ITUTINGA or FUNIL-GRANDE, the two plants with a one-point VHA
  table (1 row each in `CAD/CAD_GEOMETRIA_UHE.csv`), so the "window ignored" warning of ADR-028 S2 was not
  exercised here.
- Do the objectives now differ? The audit (FP-05, 4.502.332,7276 for three versions of the file) is not
  reproduced: the control, the audit row and the other rows give 2420590.3195, 1979381.5955, 1979371.7022 and
  2406551.2572, four different objectives for five files. The only equal pair is `3;;;0.5;0.6` and
  `3;2;5.0;0.5;0.6` (equal objective to the 4 decimals printed, and identical plane rows in the two
  `resultado_fpha_computed.csv`, compared by `diff` on the lines that do not start with `#`). The plane count of
  the audit's FURNAS (6 identical planes with and without the file) is now 5 without the row and 2 with it.
- The audit's third version (one plane) is not recorded exactly (plant and other fields unknown), so `3;1;;;` is
  an extra row, not one of the four rows asked for. It exits 1; the first `Error:` line is `Error: computed FPHA
  could not be fitted for 1 hydro plant(s) with generating units:` and the detail line is `plant 3 (FURNAS):
  fitting failed: hydro FURNAS: cannot reduce to 1 planes (2 remain after the last tolerance)`.
- The rows `3;;5.0;;`, `3;;;0.5;0.6` and `3;2;5.0;0.5;0.6` (FURNAS with 2 planes) land 44.8532, 54.7465 and
  54.7465 R$ above the constant_productivity objective of the same configuration (1979326.8490), and `3;;5.0;;`
  has a FURNAS maximum deviation of 182.958 MW against 23.285 MW in the control. The cause of both is
  `[not measured]` (see Not isolated).
- Pre, the file was read and ignored (audit FP-05, ADR-028 context), so each row would have produced the section 8
  planes and objective; that was not re-run at `d6a223d` and is `[not measured]` here.

**New rejection messages**

Derived cases outside the repository, same copy procedure; the run is the release CLI (`"$repo/target/release/labdessem"
run "$d" > run.stdout 2> run.stderr; echo "exit code $?"`). The first line of stderr is the blank line that precedes
the banner in every case; the table gives the first line that starts with `Error:` (line number in stderr).
No `RESULTADOS/` directory is created by any of the three runs.

```bash
# ticket-023 (FPHA: precomputed): plant 2 loses its unit rows, OPER_FPHA.csv has one segment row per plant 0..3
grep -v '^2;' "$d/CAD/CAD_CONJ_UHE.csv" > "$d/g.tmp" && mv "$d/g.tmp" "$d/CAD/CAD_CONJ_UHE.csv"
{ printf 'USIH;Nome;SegFPHA;Fcorrec;Rhs.mw;Varm.mw/hm3;Qtur.mw/m3s;Qlat.mw/m3s\n'; for c in 0 1 2 3; do printf '%s;PLANT-%s;1;1;0;0;0;0\n' $c $c; done; } > "$d/OPER/OPER_FPHA.csv"
# ticket-024, duplicate geometry volume (FPHA: computed): a second CAMARGOS row with the first volume, 120 hm3
printf '0;120;900.00;24.00\r\n' >> "$d/CAD/CAD_GEOMETRIA_UHE.csv"
# ticket-024, duplicate configuration row (FPHA: computed)
printf 'codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct\n3;;5.0;;\n3;2;;;\n' > "$d/CAD/CAD_FPHA_CONFIG.csv"
```

| Ticket | Case | CLI exit code | First `Error:` line (stderr) |
|---|---|---|---|
| 023 | `OPER_FPHA.csv` rows for plant 2, which has no unit row | 1 | `Error: OPER_FPHA.csv has FPHA rows for hydro plant(s) without generating units in CAD_CONJ_UHE.csv: 2 (FUNIL-GRANDE)` (line 12) |
| 024 | duplicate volume in `CAD_GEOMETRIA_UHE.csv` | 1 | `Error: plant 0 has duplicate volume 120 at lines 2 and 204 in CAD_GEOMETRIA_UHE.csv` (line 12) |
| 024 | duplicate row in `CAD_FPHA_CONFIG.csv` | 1 | `Error: I/O error: plant 3 has duplicate rows at lines 2 and 3 in CAD_FPHA_CONFIG.csv` (line 15) |

The first two errors stop the run right after `Reading input data...`; the configuration error is printed after
`Computing FPHA planes from geometry data...` and, unlike the other two, carries the `I/O error:` prefix.

**Differences**

Intended by the plan: the configuration file now changes the fit of its own plant only, the plane limit is
enforced, and inconsistent inputs stop the run with exit code 1 and a message naming the plant and the file. Every
other pre/post change found is listed here; none was explained by an isolating experiment.

- Plane count: CAMARGOS 7 to 6, FUNIL-GRANDE 4 to 3, FURNAS 6 to 5; ITUTINGA stays 3. Injected segments per
  computed run go from 20 to 17.
- Alpha: CAMARGOS 0.969607 to 0.972834 and FURNAS 0.991295 to 0.993203; ITUTINGA and FUNIL-GRANDE stay 1.000000.
  ADR-028 (a) records that the fitted planes, alpha and objective of every computed run change (6 x 6 grid
  becomes 5 x 5, window 0-100 % of CAD_UHE `[Vmin, Vmax]`); the measurement above gives the size, not the share
  of each default.
- Deviation: on the fit grid the maximum falls for CAMARGOS (2.970530 to 2.876295 MW), ITUTINGA (0.102097 to 0)
  and FURNAS (25.360739 to 23.284586 MW) and stays 0 for FUNIL-GRANDE. On the 11 x 11 check grid the maximum rises
  for all four plants (CAMARGOS 3.619181 to 3.933533, ITUTINGA 2.229353 to 2.673366, FUNIL-GRANDE 5.635867 to
  6.806018, FURNAS 39.314637 to 43.053387 MW).
- Objectives: computed +37,108.7597 R$ (+1.557 %), `O_alpha` +37,107.9310 R$ and `O_one` +47,897.7065 R$, opposite
  in sign to section 8 (-57,565.0845 R$ for computed). The round-trip gap `O_alpha - O_c` falls from +1.5744 R$
  (6.605e-7) to +0.7457 R$ (3.081e-7) and `O_c - O_one` from 2.365e-2 to 1.884e-2 relative; the post criteria
  (at most 1e-5, more than 1e-5) hold. The constant_productivity objective is unchanged (1979326.8490).
- Configuration: see the table above; one row changes one plant, the objective moves by -0.580 % (CAMARGOS to 2
  planes) or -18.2 % (FURNAS to 2 planes).
- Exit codes: A2, A3 and the three rejection cases exit 1; the solving cases exit 0 and show no infeasibility.
- Test inventory: 504 to 533 passed (+29: `labdessem-cli` unit tests +11, `fpha_e2e` +1, `labdessem-fpha` unit tests
  +5, `labdessem-fpha` `tests/regression.rs` +1, `labdessem-io` unit tests +11); 23 test binaries, failures 0 and
  ignored 2 before and after.
- Unchanged: `-D warnings` exit code 101 with the same four errors, the manifest warnings, the ignored tests, the
  constant_productivity objective, the A2 and A3 messages.
- Not comparable: wall times are single runs (0.15 s computed release, 0.05 s constant_productivity release, 0.12
  to 0.15 s per configuration case, debug E2E times in the objective table); no repeat was made, so no timing
  change is claimed.

**Not isolated**

- `[not measured]` How much of the computed objective move (+37,108.7597 R$) comes from the 5 x 5 default fit grid,
  how much from the default tolerance, the plane limit and the other Epic 03 changes under `crates/labdessem-fpha`,
  `crates/labdessem-io` and `crates/labdessem-cli` (`git diff --stat d6a223d..51751ba -- crates`: 11 files); no
  single-change build was run.
- `[not measured]` Why alpha of CAMARGOS and FURNAS moved (0.969607 to 0.972834, 0.991295 to 0.993203).
- `[not measured]` Why the fit-grid deviation of ITUTINGA fell from 0.102097 to 0, why FUNIL-GRANDE has 3 planes
  instead of 4, and why the 11 x 11 maximum rose for all four plants.
- `[not measured]` Why FURNAS with 2 planes gives an objective within 55 R$ of the constant_productivity objective
  (rows `3;;5.0;;`, `3;;;0.5;0.6`, `3;2;5.0;0.5;0.6`), including the share due to the window versus the plane count
  and the share due to the tolerance; no run isolated one field of a row, and `3;2;5.0;0.5;0.6` was not compared
  with a row of fewer fields beyond the two rows above.
- `[not measured]` The identity of the clippy diagnostic that left `labdessem-cli` (13 to 12, `collapsible_if` 8 to 7).
- `[not measured]` The objective of the four configuration rows at `d6a223d`, where the file was ignored.
- `[not measured]` The audit's own three files of FP-05 (not recorded exactly), and the audit's objective
  4.502.332,7276 (example configuration with `rede` 1 and `UCT` 1, not the fast configuration used here).
- `[not measured]` Timing: every wall time is a single run.
