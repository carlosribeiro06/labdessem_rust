# Epic 02: Correctness Fixes

## Goal

Fix A1 (alpha applied twice), A2 (generation without water) and A3 (silent physical defaults) under
the accepted decisions ADR-025, ADR-026 and ADR-027, turn every `baseline_*` characterization test of
Epic 01 into a correct-behavior regression test, prove that the documented `OPER_FPHA.csv` copy of
the computed planes reproduces the computed run, and record the post-fix evidence.

## Scope

- `crates/labdessem-fpha/src/pipeline.rs`: alpha applied once (ADR-025 option (b)).
- `crates/labdessem-cli/src/fpha_integration.rs` and `src/main.rs`: typed error for plants with units
  that cannot be fitted, plants without units excluded, post-injection validation, missing physical
  data rejected (ADR-026 option (a)+(c), ADR-027 option (a)).
- `crates/labdessem-io/src/output/fpha_computed.rs`: preamble text of `resultado_fpha_computed.csv`.
- `CHANGELOG.md` entries (`[Unreleased]`), BREAKING where the CSV contract or CLI behavior changes.
- Section 8 of `docs/auditoria/fpha-baseline-reproduction.md`.

Out of scope: `CAD_FPHA_CONFIG.csv` semantics and plane count (Epic 03), EngMax, methodology
divergences B1-B5 (Epic 04), FP-07 for precomputed plants (ticket-023).

## Entry Conditions

Every ticket of this epic assumes the recommended option of its gate. Before dispatch, the
orchestrator checks the master-plan frontmatter:

| Gate | Assumed option | Tickets |
| --- | --- | --- |
| ADR-025 | (b) remove `scale_planes` from `fit_plant`, keep `correction_factor = alpha` | 011, 012, 013 |
| ADR-026 | (a)+(c) typed error plus post-injection validation; plants without units excluded | 014, 015 |
| ADR-027 | (a) fail-fast typed error, computed FPHA only | 016 |

If a gate is accepted with another option, the affected tickets are re-refined before dispatch.

## Tickets

| Ticket | Title | Agent | Points |
| --- | --- | --- | --- |
| ticket-011-apply-alpha-once-in-fit-pipeline.md | Apply Alpha Once in the FPHA Fit Pipeline | hpc-rust-developer | 3 |
| ticket-012-align-computed-fpha-contract-with-lp.md | Align the Computed FPHA Contract with the LP | hpc-rust-developer | 2 |
| ticket-013-add-precomputed-round-trip-regression.md | Add Computed-to-Precomputed Round-Trip Regression | monorepo-test-developer | 2 |
| ticket-014-fail-on-skipped-fpha-plants.md | Fail on Plants with Units Lacking Computed FPHA | hpc-rust-developer | 3 |
| ticket-015-revalidate-system-after-fpha-injection.md | Re-validate the System After FPHA Injection | hpc-rust-developer | 2 |
| ticket-016-reject-missing-fpha-physical-data.md | Reject Missing Physical Data in Computed FPHA | hpc-rust-developer | 2 |
| ticket-017-record-post-fix-fpha-evidence.md | Record Post-Fix FPHA Evidence | monorepo-test-developer | 1 |

## Dependency Order

```
008 -> 011 -> 012 -> 013 ------------------\
009 -> 014 -> 015 --------------------------+-> 017
010 -> 016 (also after 014) ---------------/
```

Within the epic, run in the order 011, 012, 013, 014, 015, 016, 017: tickets 012, 014, 015 and 016
modify `crates/labdessem-cli/src/fpha_integration.rs`, and tickets 013 and 014 modify
`crates/labdessem-cli/tests/fpha_e2e.rs`.

## Exit Criteria

- `cargo test --workspace` exits 0; no `baseline_*` test remains in the repository.
- `cargo clippy --workspace --all-targets` reports no more warnings than the ticket-001 baseline.
- Section 8 of the evidence report compares pre-fix and post-fix values.
