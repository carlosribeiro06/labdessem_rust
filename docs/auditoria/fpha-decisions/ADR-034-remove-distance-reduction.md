# ADR-034: Removal of the distance plane-reduction criterion

Decision record for the dead and unwired code of `labdessem-fpha` found before its production engine
moves into `labdessem-model`. It supersedes only the ADR-029 S2 clause "`Distance` stays available in
the library but is not wired". ADR-029 itself is unchanged; the rest of ADR-029 S2 (the angle
criterion, consistent with ADR-028 S3) and the ADR-029 S1 plane-cap rule stand.

## Status

Accepted (2026-11-24)

## Context

Code at commit `6f10d84`.

1. `crates/labdessem-fpha/src/fitting/reduction.rs:15`: `PlaneReductionConfig::Distance { tolerance_pct,
   n_samples }`, the second variant of the reduction configuration.
2. `crates/labdessem-fpha/src/fitting/reduction.rs:103`: `pair_seed(hydro_id, entry_level_bits, ...)`,
   which seeds the `SplitMix64` sampler of `fitting/rng.rs`; only the distance branch uses it.
3. `crates/labdessem-fpha/src/fitting/reduction.rs:168-184`: `reduce_planes` takes `pf`, `bounds`,
   `hydro_id` and `entry_level_bits` only for the distance branch.
4. `crates/labdessem-fpha/src/fitting/reduction.rs:287`: the only place that creates a `Distance` value,
   in a test; `crates/labdessem-fpha/src/pipeline.rs:121-124` only doubles the tolerance of a configured one.
5. `crates/labdessem-fpha/src/pipeline.rs:112`, `:129`: the only production callers pass the configured
   variant; the cli (`crates/labdessem-cli/src/fpha_integration.rs:396`, `:407`) and the example build
   `Angle` only, so `Distance` is unreachable from a run.
6. `docs/auditoria/fpha-decisions/ADR-029-plane-count-and-milp-selection.md:235`, `:264`: the S2
   clause that keeps `Distance` in the library, not wired.

## Options

### (a)

(a) delete everything, including `Distance`, with a decision record that supersedes the ADR-029 S2 clause

- Gains: no unreachable code moves into `labdessem-model`.
- Losses: the unwired distance reduction is gone.
- Risks: none for a run, because the cli builds the angle criterion only.

### (b)

(b) delete everything except `Distance` and the code only it uses

- Gains: ADR-029 S2 stands as written.
- Losses: unreachable code (`Distance`, `pair_seed`, `fitting/rng.rs`) moves into `labdessem-model`.
- Risks: `reduce_planes` keeps four parameters that only the unreachable branch reads.

### (c)

(c) move everything as it is

- Gains: no change before the move.
- Losses: dead code moves into `labdessem-model`.
- Risks: as (b), plus the other unwired items of `labdessem-fpha`.

## Recommendation

(a).

## Decision

Accepted: option (a). Chosen by the operator on 2026-11-24.

## Consequences

- `PlaneReductionConfig` becomes `struct PlaneReductionConfig { tolerance_deg: f64 }`, and
  `reduce_planes` takes the planes and the configuration only.
- `fitting/rng.rs` and `pair_seed` are deleted.
- The `tolerance_pct` branch of ADR-029 S1 is moot.
- Reintroducing a distance criterion needs a new decision record, which reopens ADR-028 S3.
