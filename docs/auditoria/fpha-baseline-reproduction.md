# FPHA Baseline Reproduction

This report records the state of the workspace before any FPHA correction: the toolchain, the build,
test, lint and format status of commit `4ac43e0` (develop, v0.12.0 plus the model audit report), and
three fast derived-case runs of the `labdessem` CLI. Every later FPHA ticket compares against these
numbers, so a failure or a number change after a ticket is attributable to that ticket and not to the
starting point. Sections 5 to 8 are placeholders that later tickets fill in. Nothing was fixed while
measuring; the official example `examples/24Barras` was never modified or executed.

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

Pending: ticket-007.

## 6. A2 skipped plants

Pending: ticket-007.

## 7. A3 physical defaults

Pending: ticket-007.

## 8. Post-fix comparison

Pending: ticket-017.
