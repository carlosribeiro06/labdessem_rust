# ADR-025: Apply the FPHA correction factor alpha exactly once in computed mode

Decision brief for defect A1 (audit FP-02, with FP-08 and decision D2). Requirements R1, R2, R9 of
`plans/fpha-corrections/00-master-plan.md`. Option names follow the ADR-025 entry of the master-plan
frontmatter.

## Status

Proposed

## Context

**Defect.** In `FPHA = "computed"` mode the correction factor alpha reaches the LP twice for
gamma_0, gamma_V and gamma_Q, and once for gamma_S:

1. `crates/labdessem-fpha/src/pipeline.rs:84-85`: `fit_plant` computes `alpha` and returns
   `planes = scale_planes(&raw_planes, alpha)` (first multiplication of gamma_0..gamma_Q).
2. `crates/labdessem-fpha/src/pipeline.rs:88`: `fit_gamma_s_for_planes` then fits gamma_S on the
   exact production function and overwrites it, without alpha (audit FP-08).
3. `crates/labdessem-fpha/src/pipeline.rs:91` and `:101`: plane reduction and the reported deviation
   run on these alpha-scaled planes.
4. `crates/labdessem-cli/src/fpha_integration.rs:315`: `convert_to_fpha_segments` sets
   `correction_factor: result.alpha`.
5. `crates/labdessem-model/src/constraints/hydro.rs:198` (`build_hydro_fpha_constraints`, lines
   227-252): the model multiplies every coefficient and the RHS by `correction_factor` (second
   multiplication).

The effective envelope is `alpha^2 * FPHA_0` on gamma_0..gamma_Q and `alpha * gamma_S` on the lateral
flow term, instead of `alpha * FPHA_0` on all four. The test `convert_single_plane`
(`crates/labdessem-cli/src/fpha_integration.rs:410`) requires `correction_factor == alpha` and pins
the current behaviour (audit FP-R03). The precomputed path is not affected: it reads raw coefficients
and `Fcorrec` from `OPER/OPER_FPHA.csv` and the model multiplies once.

`resultado_fpha_computed.csv` (`crates/labdessem-io/src/output/fpha_computed.rs`) prints the
alpha-scaled gamma_0..gamma_Q, the unscaled gamma_S, and an `alpha` column described as
"fator de correcao aplicado aos coeficientes", so the documented `OPER_FPHA.csv` copy uses
`Fcorrec = 1`.

**Reproduced numbers** (`docs/auditoria/fpha-baseline-reproduction.md`, commit 70cbbba, sections 5
and 7a; debug build, fast configuration `rede` 0, `UCT` 0, `FCF` 1, `EngMax` 0). Deviation cells
are absolute magnitudes, `max_abs / mean_abs / rms`, in MW, against the exact production function:

| Code | Plant | alpha | Planes | `E_LP / intended` | Reported deviation, fit grid | LP-based deviation, fit grid |
|---|---|---|---|---|---|---|
| 0 | CAMARGOS | 0.969607 | 6 | 0.969607257 | 3.142964 / 0.953364 / 1.291594 | 2.753641 / 1.042563 / 1.414023 |
| 1 | ITUTINGA | 1.000000 | 3 | 1.000000000 | 0.102097 / 0.032232 / 0.055914 | 0.102097 / 0.032232 / 0.055914 |
| 2 | FUNIL-GRANDE | 1.000000 | 4 | 1.000000000 | 0.000000 / 0.000000 / 0.000000 | 0.000000 / 0.000000 / 0.000000 |
| 3 | FURNAS | 0.991295 | 6 | 0.991295163 | 25.360739 / 9.047101 / 11.706933 | 21.078021 / 9.088040 / 11.678164 |

- `E_LP / intended` (the LP generation cap at `s = 0` over `alpha * min_k U_k`, with `U_k` the
  unscaled planes) equals alpha at every qualifying fit-grid node, asserted within 1e-9: the LP
  applies one alpha more than intended. The ratio is alpha by construction of the check (section 5),
  so it carries no spread information.
- For the two plants with alpha different from 1, the deviation the pipeline reports differs from the
  deviation of the envelope the LP actually uses (3.142964 against 2.753641 MW; 25.360739 against
  21.078021 MW maximum absolute).
- End to end: computed objective `O_c` = 2,441,046.6443 R$; the `OPER_FPHA.csv` copy of
  `resultado_fpha_computed.csv` with `Fcorrec = alpha` gives `O_alpha` = 2,441,052.2402 R$
  (`|O_alpha - O_c| / |O_c|` = 2.292e-6); the copy with `Fcorrec = 1` (the one the current CSV
  preamble documents) gives `O_one` = 2,383,746.0919 R$, so `O_c - O_one` = +57,300.5524 R$
  (2.347e-2 relative). The residual of `O_alpha` is consistent with the CSV rounding of the planes,
  not separately isolated.
- Audit comparison (section 7a): alpha matches the audit within 1e-6 for all four plants; the audit's
  gap is +3,125.65 R$ (about 0.07 %, official configuration), the reproduced one +57,300.55 R$
  (2.35 %, fast configuration). Same sign (computed above the `Fcorrec = 1` copy); only sign and
  order of magnitude are comparable.

## Methodology reference

Sources: `~/.claude/skills/dessem/references/42-metodologia-hidreletricas.md` section 16 (MM 22.3.0
§9.16.11-9.16.12, p. 71-76 / PDF 97-102; Libs §15.6.1), `~/.claude/skills/dessem/references/23-saidas-diagnostico-b.md`
§2.2 (AVL_FPHA1, MU §V.11.4; Libs §15.6) and `docs/design/fpha-formulation.md`.

MM §9.16.11 step 4 (eq. 9.32) defines the correction as one multiplicative factor on the whole convex
hull, `FPHA = alpha_FPHA * FPHA_0`, with alpha chosen by least squares against the exact FPH; Libs
§15.6.1 gives the closed form `alpha = sum(FPHA_0 * FPH) / sum(FPHA_0^2)`. Alpha is applied once, to
the whole function.

Libs §15.6.1 writes the LP constraint as
`GH_i <= alpha_i * [gamma_0^k + gamma_V^k V_i + gamma_Q^k Q_i + gamma_S^k (Q_lat,i - Q_lat_hat,i)]`:
alpha multiplies all four terms, gamma_S included, exactly once. MM §9.16.11 eq. 9.33 shows the cut
without alpha and without the offset `Q_lat_hat` (divergence item 4 of `42-metodologia-hidreletricas.md`);
whether the MM coefficients of 9.33 already carry alpha is not stated `[to confirm]`.

MM §9.16.11 orders the steps as hull (step 3), alpha (step 4) and lateral-flow secant (step 5). Neither
the MM nor the Libs §15.6.1 text summarised in section 16 states whether the secant targets the exact
FPH or `FPH / alpha` before alpha is applied to gamma_S `[to confirm]`. This is the open point behind
FP-08: under the Libs form, a gamma_S fitted directly on the exact FPH and then multiplied by alpha
carries a small alpha bias on the lateral-flow slope.

For the file convention, `23-saidas-diagnostico-b.md` §2.2 (MU §V.11.4, Libs §15.6) records that the
DESSEM output AVL_FPHA1 prints each cut as `Rhs`, `Varm`, `Qtur`, `Qlat` with a separate `Fcorrec`
column (p.u.) and that, in the Libs, alpha multiplies the whole cut. The labdessem input
`OPER/OPER_FPHA.csv` (`USIH;Nome;SegFPHA;Fcorrec;Rhs.mw;Varm.mw/hm3;Qtur.mw/m3s;Qlat.mw/m3s`) follows
that convention: raw coefficients plus `Fcorrec`, multiplied once by the model, as
`docs/design/fpha-formulation.md` states (`beta = volume coefficient x correction factor`, and so on).
That the AVL_FPHA1 coefficients are printed without alpha is inferred from the Libs form; the real
samples quoted in `23-saidas-diagnostico-b.md` show `Fcorrec = 1.00000`, which cannot discriminate,
and no deck was available on this machine to check a plant with `Fcorrec` different from 1
`[to confirm]`.

No contradiction found: the audit's description (precomputed reads raw coefficients and `Fcorrec`
separately and the model multiplies once, as in the Libs) agrees with Libs §15.6.1 and with the
AVL_FPHA1 layout of MU §V.11.4. The only methodological gap is the secant target above `[to confirm]`,
which none of the three options settles by itself.

## Options

### (a1)

`correction_factor = 1.0` in `convert_to_fpha_segments`; planes stay alpha-scaled.

- Gains: one-line change in `fpha_integration.rs`; plane reduction and the fit pipeline are untouched,
  so the plane sets are bit-identical to today; the reported deviation becomes the LP deviation on
  `s = 0` (the reported column already describes the alpha-scaled planes); `resultado_fpha_computed.csv`
  values do not change.
- Losses: gamma_S stays without alpha (FP-08 persists), so the computed LP diverges from the Libs form
  on the lateral-flow term; the `alpha` column becomes informational only; the computed planes do not
  follow the `OPER_FPHA` raw-coefficients-plus-`Fcorrec` convention.
- Risks: a user who copies the CSV into `OPER_FPHA.csv` with `Fcorrec = alpha` (the Libs/AVL_FPHA1
  reading) silently reproduces the alpha^2 envelope; the post-fix objective is expected near the
  `Fcorrec = 1` copy (`O_one` = 2,383,746.0919 R$ in the fast configuration), an expectation, not a
  measurement.
- resultado_fpha_computed.csv: values unchanged; the preamble must say that gamma_0..gamma_Q already
  include alpha, gamma_S does not, and `alpha` is informational; not BREAKING for values, but the
  asymmetric gamma_S must be documented.
- OPER_FPHA.csv copy: `Fcorrec = 1`, other columns copied as printed.
- Tickets affected: ticket-011 is rewritten (no `pipeline.rs` change; the `lp_envelope_tests.rs`
  tests pin `E_LP == min_k P_k` with gamma_S unscaled); ticket-012 rewritten (one-line fix, test
  asserts `correction_factor == 1`, preamble and CHANGELOG `### Fixed` only); ticket-013 rewritten
  (round trip with `Fcorrec::One`, sensitivity control with `Fcorrec::Alpha`).

### (a2)

`correction_factor = 1.0` and scale gamma_S by alpha after the secant fit.

- Gains: alpha reaches all four coefficients exactly once, as in the Libs form (Libs §15.6.1); small
  change (one line in `fpha_integration.rs`, one scaling of gamma_S in `pipeline.rs` after line 88);
  gamma_0..gamma_Q values in the CSV are unchanged.
- Losses: CSV planes stay pre-multiplied and the `alpha` column is informational; the computed output
  still does not follow the `OPER_FPHA` convention; gamma_S values in the CSV change.
- Risks: as in (a1), a copy with `Fcorrec = alpha` reproduces a double alpha; scaling gamma_S before
  `reduce_planes` can change which planes survive the angle/distance reduction for plants with alpha
  different from 1 (plane normals are not scale invariant), so plane counts can change; the touched
  `pipeline.rs` is an HPC-fenced path and needs the bit-exact regression evidence.
- resultado_fpha_computed.csv: gamma_S column changes (now includes alpha); gamma_0..gamma_Q unchanged;
  preamble must say all four coefficients already include alpha and `alpha` is informational; a
  BREAKING change for the gamma_S column.
- OPER_FPHA.csv copy: `Fcorrec = 1`, other columns copied as printed.
- Tickets affected: ticket-011 rewritten (scale gamma_S after the secant instead of removing
  `scale_planes`; tests pin `E_LP == min_k P_k` for s in {0, Q_max/2, Q_max}); ticket-012 rewritten
  (`correction_factor == 1`, preamble, CHANGELOG `### Fixed` plus a BREAKING note for gamma_S);
  ticket-013 rewritten (round trip with `Fcorrec::One`).

### (b)

Remove `scale_planes` from `fit_plant` and keep `correction_factor = alpha`.

- Gains: matches the Libs form (Libs §15.6.1) and the `OPER_FPHA` / AVL_FPHA1 `Fcorrec` convention
  (MU §V.11.4): raw planes plus alpha, applied once by the model to all four coefficients, gamma_S
  included (closes FP-08 as the audit frames it); the CSV becomes a direct `OPER_FPHA.csv` copy with
  `Fcorrec = alpha`; the reported deviation describes `alpha x planes`, i.e. the LP envelope; the
  `alpha` column becomes load-bearing.
- Losses: more change (pipeline stage order, deviation call, comments, tests in three files);
  `resultado_fpha_computed.csv` values of gamma_0..gamma_Q change for every plant with alpha different
  from 1.
- Risks: removing `scale_planes` changes the input of `reduce_planes`, and plane normals are not
  scale invariant, so the angle/distance reduction can keep a different plane set (plane counts and
  objective can move beyond the pure alpha effect); `pipeline.rs` is an HPC-fenced path and needs the
  bit-exact regression evidence; downstream consumers that read the gamma columns as final MW
  coefficients silently get values 1/alpha too large; if ADR-033 later moves the alpha computation
  after reduction, the stage order is revisited (master-plan rework trigger).
- resultado_fpha_computed.csv: BREAKING change of semantics: gamma_0..gamma_S become unscaled, `alpha`
  is the factor the model applies once, the deviation columns describe `alpha x planes`; column names,
  order and number format unchanged.
- OPER_FPHA.csv copy: `Fcorrec = alpha`, `USIH = codigo_usina`, `SegFPHA = plane_index + 1`,
  `Rhs.mw = gamma_0`, `Varm.mw/hm3 = gamma_v`, `Qtur.mw/m3s = gamma_q`, `Qlat.mw/m3s = gamma_s`.
- Tickets affected: ticket-011, ticket-012 and ticket-013 proceed as written (they assume (b)).

## Recommendation

(b). It is the only option that makes the computed output follow the same convention as the
precomputed input and the DESSEM/Libs form (Libs §15.6.1; AVL_FPHA1 layout, MU §V.11.4), so one
documented copy rule (`Fcorrec = alpha`) round-trips and is regression-tested (ticket-013). It applies
alpha once to all four coefficients, which (a1) does not, and it keeps the `alpha` column meaningful,
which (a1) and (a2) do not. The reported deviation then describes the envelope the LP uses. The cost
is a BREAKING change to the `gamma_*` columns of `resultado_fpha_computed.csv` and a possible change of
the reduced plane set; both are bounded by the regression tests of tickets 011-013 and by the
post-fix evidence of ticket-017. The master plan classifies the decision as two-way; the CSV contract
is the part that is costly to change twice, which argues for settling it now on the DESSEM
convention. The open methodological point (secant target, `[to confirm]` above) is independent of the
choice and can be revisited without changing the contract.

## Decision

PENDING

## Consequences

- If (a1) is chosen:
  - ticket-011: rewritten; no `pipeline.rs` change; `baseline_a1_*` tests replaced by tests that pin
    `E_LP == min_k P_k` at `s = 0` and the reported deviation equal to the LP deviation.
  - ticket-012: rewritten; `correction_factor: 1.0`, test asserts it; preamble documents pre-scaled
    gamma_0..gamma_Q, unscaled gamma_S, informational `alpha`.
  - ticket-013: rewritten; round trip with `Fcorrec::One`, sensitivity control with `Fcorrec::Alpha`.
  - `CHANGELOG.md`: `### Fixed` (alpha applied twice); no BREAKING entry; FP-08 recorded as open.
- If (a2) is chosen:
  - ticket-011: rewritten; scale gamma_S by alpha after the secant fit, before reduction; tests pin
    `E_LP == min_k P_k` for s in {0, Q_max/2, Q_max}; HPC-fence evidence required.
  - ticket-012: rewritten; `correction_factor: 1.0`; preamble documents all-pre-scaled planes.
  - ticket-013: rewritten; round trip with `Fcorrec::One`.
  - `CHANGELOG.md`: `### Fixed` (FP-02, FP-08) and a `**BREAKING**:` entry for the gamma_S column.
- If (b) is chosen:
  - ticket-011, ticket-012, ticket-013: proceed as written.
  - `CHANGELOG.md`: `### Fixed` (FP-02, FP-08) and `### Changed` `**BREAKING**:` entry for
    `resultado_fpha_computed.csv` with the copy rule `Fcorrec = alpha` (ticket-012 requirement 4).
- In all cases: the test `convert_single_plane` is replaced by a test on the product
  `correction_factor x coefficient` (audit FP-R03); `docs/design/fpha-formulation.md` is brought in
  line later (FP-11); the master-plan ADR-025 entry is set to `accepted` by the orchestrator.
