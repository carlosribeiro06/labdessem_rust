# ADR-030: Volume used in the FPHA rows (end of period or period mean)

Decision brief for methodology divergence B1 (audit FP-06, decision D9 of
`docs/auditoria/2026-11-05-auditoria-modelo.md`): every FPHA row uses the end-of-period volume `V[t+1]`,
while the Libs form of the same row and the project's own EngMax head volume (ADR-006) use the period
mean. Requirement R7 of `plans/fpha-corrections/00-master-plan.md`. Option names follow the ADR-030 entry
of the master-plan frontmatter. Context only, not options here: ADR-025 (accepted) applies alpha once;
ADR-028 (accepted) fixes the fit domain (volume window as a fraction of the `CAD_UHE.csv` `[Vmin, Vmax]`);
ADR-029 (accepted) caps the plane count and keeps every plane in every solve stage. The turbining axis,
tailrace families and alpha points are ADR-031, ADR-032 and ADR-033.

## Status

Proposed

## Context

**Current behaviour.** Code at HEAD `8b8afaa` (branch `fix/fpha-corrections`).

1. `build_hydro_fpha_constraints` (`crates/labdessem-model/src/constraints/hydro.rs`) builds one `<=` row
   per plant, period and `plant.fpha_segments` entry, named `hydro_fpha[p=..,seg=..,t=..]`. The volume
   term is `variables.hydro_volume[plant_entry_idx * (horizon + 1) + period + 1]`, the end-of-period slot,
   with coefficient `-correction_factor * volume_coefficient`. The turbining term of each unit is divided
   by `flow_conversion = period_duration_hours * 0.0036` (the turbining variable is in hm3 per period).
   The spillage term carries the lateral-flow coefficient and is divided the same way. The right-hand side
   is `correction_factor * rhs`, so every coefficient is multiplied by `segment.correction_factor`.
   Precomputed segments (`OPER/OPER_FPHA.csv`) and computed segments go through this builder.
2. The volume variables (`Variables` in `crates/labdessem-model/src/variables.rs`) have `horizon + 1`
   slots per plant. Slot 0 is fixed to `reservoir.initial_volume_hm3` (`VolInic.hm3` of `CAD_UHE.csv`).
   Slot `t + 1` is the volume at the end of period `t`.
3. `hydro_generation_max_at_point_mw` (`crates/labdessem-io/src/output/helpers.rs`, column
   `GeracaoMaxPontoMW` of `resultado_hidreletricas.csv`) evaluates the same planes at
   `hydro_volume_name(&plant.name, period + 1)`, the end-of-period volume, with the plant's summed maximum
   turbining and no spillage term.
4. EngMax uses the period mean. `period_average_volume_hm3` (`crates/labdessem-simulation/src/engmax.rs`)
   averages slots `period` and `period + 1` (ADR-006, `docs/design/engmax-formulation.md`). So two
   consumers of the same reservoir head use different volumes today.
5. The FPHA probe (`envelope_row` in `crates/labdessem-cli/src/fpha_integration/probe.rs`, used by
   `lp_envelope_tests.rs`) reads exactly one `hydro_volume[` term per row (`single_coefficient`). Its
   comment says the volume term references `V[t+1]`.
6. ADR-028 (accepted): the fit volume window is a fraction of the `CAD_UHE.csv` `[Vmin, Vmax]` interval
   (`build_fpha_config` in `crates/labdessem-cli/src/fpha_integration.rs`, `fit_context` in
   `crates/labdessem-fpha/src/pipeline.rs`). The one-point VHA plants ITUTINGA and FUNIL-GRANDE (`Tipo`
   `FD`, `Vmin = Vmax`) ignore the window and keep the 1 hm3 half-width synthesis of
   `resolve_fitting_bounds` (`crates/labdessem-fpha/src/fitting/grid.rs`).

**Baseline** (`docs/auditoria/fpha-baseline-reproduction.md`, section 9, commit `51751ba`, fast
configuration: `opcao_execucao` 2, `rede` 0, `UCT` 0, `UCH` 0, `FCF` 1, `EngMax` 0). Planes 6 / 3 / 3 / 5
(CAMARGOS, ITUTINGA, FUNIL-GRANDE, FURNAS). Alpha 0.972834 / 1.0 / 1.0 / 0.993203. Computed objective
2420590.3195 R$; `Fcorrec = alpha` copy 2420591.0652 R$; constant_productivity 1979326.8490 R$.

**Static measurement.** One release CLI run at HEAD `8b8afaa` (`cargo build --release -p labdessem-cli`)
on a derived copy of `examples/24Barras` in the session scratchpad, outside the repository.
`examples/24Barras` was neither edited nor executed. The run reproduces section 9: exit code 0, planes
6 / 3 / 3 / 5, `Objective:   2420590.3195`.

```bash
d=$(mktemp -d); cp -r examples/24Barras/CAD examples/24Barras/OPER "$d"/
# study_config.json: {"opcao_execucao": 2, "rede": 0, "UCT": 0, "UCH": 0, "TON_Residual": 0,
#                     "FPHA": "computed", "FCF": 1, "EngMax": 0}
target/release/labdessem run "$d" > run.stdout 2> run.stderr
python3 -I volume_slot_check.py "$d"
python3 -I fpha_volume_reference.py "$d"
```

Both scripts were kept in the scratchpad, not in the repository:

- `volume_slot_check.py` checks the `VolumeHM3` column. For CAMARGOS, which has no upstream plant, it
  replays the water balance from `VolInic.hm3` = 786.09 with `AfluenciaHM3 - TurbinamentoHM3 -
  VertimentoHM3 - DesvioHM3 - RetiradaHM3 + BombeamentoHM3` (plant-total rows, `Conjunto` 99).
  It prints `CAMARGOS periods=48 max |V0+balance - VolumeHM3| = 1.000e-06 hm3`. So `VolumeHM3` of
  period `t` is the end-of-period volume (slot `t + 1`). The writer agrees: `write_hydro_csv` in
  `crates/labdessem-io/src/output/hydro.rs` reads slot `last_internal_period + 1`.
- `fpha_volume_reference.py` evaluates the LP envelope
  `min_k alpha * (gamma_0 + gamma_v * V + gamma_q * Q + gamma_s * S)` for every plant with units and
  every period. The planes come from `RESULTADOS/resultado_fpha_computed.csv`. Q and S are
  `TurbinamentoM3s` and `VertimentoM3s` of the plant-total rows of
  `RESULTADOS/resultado_hidreletricas.csv`. V is evaluated once at the end-of-period volume (`VolumeHM3`)
  and once at the period mean (`VolInic.hm3` of `CAD_UHE.csv` as the start volume of the first period).
- Consistency check: `GeracaoMW` minus the end-of-period envelope is at most +1.18e-4 MW (the rounding of
  the 6-decimal CSV coefficients). So the rows are binding at the end-of-period volume, as item 1 says.

| Plant | Periods | Max abs. difference, mean vs end (MW) | Period of max | Mean minus end (MW) | Max abs. volume change per period (hm3) |
|---|---|---|---|---|---|
| CAMARGOS | 48 | 0.000000 | none | 0.000000 | 2.733102 |
| ITUTINGA | 48 | 0.000000 | none | 0.000000 | 0.000000 |
| FUNIL-GRANDE | 48 | 0.000000 | none | 0.000000 | 0.000000 |
| FURNAS | 48 | 0.011043 | 9 | -0.011043 | 1.416582 |

Measured effect: largest absolute change of the FPHA LP envelope at the solved point when V moves from the end-of-period volume to the period mean, per plant: CAMARGOS 0.000000 MW, ITUTINGA 0.000000 MW, FUNIL-GRANDE 0.000000 MW, FURNAS 0.011043 MW (period 9); maximum over the case 0.011043 MW.

- CAMARGOS shows 0 because its binding planes at its solved volumes (about 782 to 786 hm3, near
  `Vmax` 792) have `gamma_v` = 0 (plane 4) or about 0 (plane 5). The analytic bound
  `alpha * max_k |gamma_v| * max |dV| / 2` is 0.0719 MW for CAMARGOS and 0.0110 MW for FURNAS. FURNAS
  reaches its bound on plane 0.
- ITUTINGA and FUNIL-GRANDE have `gamma_v` = 0 on every plane and a constant volume (`Vmin = Vmax`). The
  choice cannot affect them.
- The 24Barras periods are 0.5 h long (48 periods). The difference is proportional to the volume change
  per period, so it grows with longer periods and with larger flows relative to the reservoir.

Measured effect: the audit's "at most 0.25 MW" (FP-06) was measured on another configuration and commit, and is not reproduced here `[not measured]`.

Measured effect: the re-optimized objective under (b) is `[not measured]`. The experiment needs the (b) row builder (S1 below). It would re-run the section 9 fast computed case and compare the objective with 2420590.3195 R$.

Measured effect: the change of `GeracaoMaxPontoMW` under (b) is `[not measured]`. It is bounded by the same `alpha * max_k |gamma_v| * |dV| / 2` (0.0719 MW CAMARGOS, 0.0110 MW FURNAS on this run).

## Methodology reference

No manual PDF and no ONS deck are on this machine, so every claim below comes from the `dessem` skill
references (`~/.claude/skills/dessem/references/`), which cite MM 22.3.0 (same chapter 9 formulation and
equation numbers in MM 22.4.1, `42-metodologia-hidreletricas.md` section 20). No deck evidence is quoted.

- MM (9.33), `42-metodologia-hidreletricas.md` section 16 "Restricao no PL/MILP" (MM 22.3.0 §9.16.12,
  p. 71-76 / PDF 97-102):
  `GH_i^t <= gamma_0 + gamma_V V_i^t + gamma_Q Q_i^t + gamma_Qlat Qlat_i^t`, `k = 1..NPF_i`. The volume
  is `V_i^t` with no averaging. The same section calls it a physical constraint.
- `V_i^t` is the end-of-period volume, `42-metodologia-hidreletricas.md` section 12 (MM §9.13, p. 50 /
  PDF 76): the water balance (9.13) is `V_i^t = V_i^{t-1} + varsigma_t [inflows - outflows]`. So `V_i^t`
  of (9.33) read literally is the volume at the end of period `t`. That is labdessem's slot `t + 1`, which
  is what `build_hydro_fpha_constraints` uses. Whether the MM means the literal end-of-period variable or
  uses `V_i^t` as shorthand in (9.33) is not stated in the reference `[to confirm]`.
- Libs form, `42-metodologia-hidreletricas.md` section 16 "Restricao no PL/MILP" (Libs §15.6.1):
  `GH_i <= alpha_i [gamma_0 + gamma_V V_i + gamma_Q Q_i + gamma_S (Qlat_i - Qlat_hat_i)]`, with `V` = the
  **period-mean volume**.
- The divergence is recorded in the skill, `42-metodologia-hidreletricas.md` "Divergencias e duvidas"
  item 4: (9.33) has neither alpha nor the `Qlat_hat` offset; the Libs form has both and uses the mean
  volume. So MM and Libs **do differ** on the volume in the sources read.
- The MM uses an explicit mean where it means one, `42-metodologia-hidreletricas.md` section 14 (MM §9.15,
  p. 56-58 / PDF 82-84): the linear evaporation (9.19) is applied to the period-mean volume
  `(V^{t-1} + V^t) / 2`. PDO_OPER_EVAP compares it with the exact evaporation at the mean volume of the
  final solution (MM §9.15.3; MU §V.6.3).
- Official DESSEM builds its FPHA through the Libs by default, `42-metodologia-hidreletricas.md` section 16
  "Abordagens e opcoes" and `60-versoes-dessem.md` (MU 22.3.2 §III.6.3.5): "FPHA pelas LIBS" is the
  default, `FPHADESSEM` in DESSOPC returns to DESSEM's own construction, and `ILIBS` is mandatory without
  it. The sources say the Libs **compute the planes**. They do not say whether DESSEM 22.0.2 (the ONS
  decks of 2026) then writes the LP row with the Libs mean-volume form or with (9.33) `[to confirm]`.
- Indirect evidence from a diagnostic output, `23-saidas-diagnostico-b.md` section 2.4 (MU §V.11.5;
  MM §9.16.12): AVL_DESVFPHA reports, per plant and period of the final solution, `Varm Med` (mean total
  volume) next to `Ghid (FPH)`, `Ghid (PL)` and `Ghid (FPHA`. This suggests DESSEM evaluates its FPHA at
  the period-mean volume. Whether `Ghid (FPHA` is computed at `Varm Med` is not stated `[to confirm]`.
  A numeric check would settle it: evaluate the AVL_FPHA1 cuts of the 032026 full case (DESSEM 22.0.2)
  at `Varm Med` and at the period's final volume (PDO_HIDR) and compare with `Ghid (FPHA`. The check
  needs the deck, which is not on this machine.
- Related point outside ADR-030, `23-saidas-diagnostico-b.md` section 2.2 (MU §V.11.4): the idessem
  name of the AVL_FPHA1 `Varm` coefficient is `coeficiente_volume_util`. Whether a DESSEM cut multiplies
  useful or total volume (which shifts `Rhs` by `gamma_V * Vmin` when cuts are copied into
  `OPER_FPHA.csv`) is `[to confirm]`. It does not change the end-versus-mean choice.

## Options

### (a)

(a) keep the end-of-period volume V[t+1] (MM eq. 9.33)

- Gains: no code, test or output change. The rows match the literal reading of MM (9.33) with `V_i^t` from
  (9.13). The section 9 objectives, the ADR-025 round-trip test (`O_alpha - O_c`) and the `lp_envelope`
  probe stay as they are.
- Losses: the rows keep diverging from the Libs form (mean volume, Libs §15.6.1), which the sources say is
  the default FPHA builder of official DESSEM, and from the period-mean convention that the MM uses for
  evaporation (9.19). The asymmetry with EngMax (ADR-006, mean) stays in the codebase: FPHA and EngMax
  take the same reservoir head at different volumes.
- Risks: if the `[to confirm]` check shows that DESSEM 22.0.2 evaluates its rows at the mean, labdessem
  stays systematically offset from the official model by `alpha * gamma_v * dV / 2` per period (0.011 MW
  on 24Barras, larger for longer periods or larger volume changes). Generation in period `t` is bounded
  by the head at the end of `t`, so the drawdown within the period is valued at the final head only.
- FPHA rows: unchanged (one volume term on slot `t + 1`, coefficient `-alpha * gamma_v`).
- Precomputed decks: `OPER/OPER_FPHA.csv` `Varm.mw/hm3` keeps meaning a coefficient on the end-of-period
  volume. Cuts copied from a DESSEM AVL_FPHA1 are applied at a volume that may differ from the one
  DESSEM uses `[to confirm]`.
- Tickets affected: no implementation ticket. Ticket-029 documents the end-of-period volume as current
  behaviour, the divergence from the Libs form and from ADR-006, and the open `[to confirm]` items.
  Ticket-030 states that `GeracaoMaxPontoMW` uses the end-of-period volume.

### (b)

(b) use the period-mean volume (V[t] + V[t+1]) / 2 (Libs form)

- Gains: the rows follow the Libs form, which the sources say is the default FPHA builder of official
  DESSEM, and the MM's own mean convention for evaporation (9.19). FPHA and EngMax share one head-volume
  definition (ADR-006), so the codebase asymmetry is removed. Generation in period `t` responds to the
  head during the period instead of the head at its end. The change is small and confined: one row
  builder, one output helper and their tests. It is a two-way door.
- Losses: the rows no longer match the literal (9.33). Every computed and precomputed objective changes
  by a small amount `[not measured]`, so the section 9 objectives stop being the current baseline. The
  first-period row gains a constant on its right-hand side (S1).
- Risks: the probe `envelope_row` reads one `hydro_volume[` coefficient per period-1 row. Under S1 it would
  read `alpha * gamma_v / 2` and the shifted right-hand side, so `lp_envelope_tests.rs` and the probe must
  be updated with the builder, or they fail or measure a different envelope. Each FPHA row now couples two
  volume slots, so the volume duals (water values) and the CMO move slightly `[not measured]`. If the
  `[to confirm]` check shows that DESSEM 22.0.2 uses the end-of-period volume, (b) would be the divergence.
- FPHA rows: periods `t >= 1` have two volume terms, `-alpha * gamma_v / 2` on slots `t` and `t + 1`.
  Period 0 has one term, `-alpha * gamma_v / 2` on slot 1, and the right-hand side
  `alpha * (rhs + gamma_v * VolInic / 2)`. Row count, names and sense do not change.
- Precomputed decks: `OPER_FPHA.csv` keeps its columns and values. `Varm.mw/hm3` is now read as a
  coefficient on the period-mean volume, which is the Libs reading of a DESSEM cut. Precomputed and
  computed segments change alike because they share the builder.
- Tickets affected: an implementation ticket ([ADD], S4) changes `build_hydro_fpha_constraints`,
  `hydro_generation_max_at_point_mw`, the probe `envelope_row` and its tests, and the FPHA model tests.
  Ticket-029 documents the mean volume (and ADR-006 symmetry). Ticket-030 states that `GeracaoMaxPontoMW`
  uses the period-mean volume. The section 9 objectives are re-recorded by that ticket
  `[not measured]` here.

## Recommendation

(b). The measured effect is negligible on 24Barras (at most 0.011043 MW, FURNAS), so the choice is made on
consistency, not on size, and four of the sources read point to the mean:

- the Libs form (Libs §15.6.1), the default FPHA builder of official DESSEM (MU 22.3.2 §III.6.3.5);
- the MM's explicit mean convention for evaporation (9.19);
- the AVL_DESVFPHA diagnostic, which reports `Varm Med` next to `Ghid (FPHA`;
- labdessem's own EngMax (ADR-006).

Only the literal reading of MM (9.33) supports (a), and the sources do not say whether `V_i^t` there is
shorthand `[to confirm]`. (b) also removes an architectural asymmetry (two consumers of one reservoir
head with two volumes), and it is a two-way door.

Rework trigger: if the 032026 full-case check (AVL_FPHA1 cuts at `Varm Med` against `Ghid (FPHA`) shows
that DESSEM 22.0.2 evaluates the FPHA at the end-of-period volume, the deck wins and (a) is reopened.

S1: The LP form of (b) splits the volume coefficient in half. For period `t >= 1` the row is
`GH - alpha*gamma_q*Q - alpha*gamma_s*S - (alpha*gamma_v/2)*V[t] - (alpha*gamma_v/2)*V[t+1] <= alpha*gamma_0`
(slots `t` and `t + 1` of `hydro_volume`). For period 0, slot 0 is the fixed initial volume, so it moves
to the right-hand side: one volume term `-(alpha*gamma_v/2)*V[1]` and
`rhs = alpha*(gamma_0 + gamma_v*VolInic/2)`. Keeping the fixed slot-0 variable in the row is
algebraically equivalent, but it keeps a fixed column in every period-0 row and gives the probe a
second volume term to interpret, so it is not recommended. The turbining and spillage terms do not
change. The probe `envelope_row` must accept the split (sum the volume coefficients and evaluate the
constant against `VolInic`) so that `lp_envelope_tests.rs` keeps measuring the same envelope.

S2: (b) applies to precomputed and computed segments alike, because both go through
`build_hydro_fpha_constraints`. There is no computed-only variant and no new `OPER_FPHA.csv` column.
`GeracaoMaxPontoMW` follows: `hydro_generation_max_at_point_mw` evaluates the planes at the mean of
`hydro_volume_name(.., period)` and `hydro_volume_name(.., period + 1)`. Both slots are already in
`variable_values` (`output_prep.rs` inserts slots `0..=horizon`), so the column stays consistent with
the rows. `VolumeHM3` stays the end-of-period volume, since it is a storage report, not a head.

S3: ADR-028 is unchanged. Its window (`min_volume_pct`/`max_volume_pct` of `CAD_FPHA_CONFIG.csv`, a fraction of the `CAD_UHE.csv`
`[Vmin, Vmax]`) and
its reference describe the fit domain in volume, the axis on which the planes are fitted, not which
period volume the row reads. A period-mean volume lies between two feasible slot volumes, so it stays
inside `[Vmin, Vmax]`. The one-point plants (ITUTINGA, FUNIL-GRANDE, `Vmin = Vmax`, `gamma_v` = 0 on
every plane) are not affected: their slot volumes are equal and the volume term is 0 under either
option.

S4: Deliver an accepted (b) through an implementation ticket added to this plan ([ADD], recommended)
through the Scope Change Approval protocol. Its scope is the builder, the output helper, the probe and
its tests, and the model test. It should run before ticket-029 and re-record the section 9 objectives.
The change is small and two-way, and leaving it to a follow-up plan would make ticket-029 document a
planned change instead of current behaviour. Alternative: a follow-up plan. Then ticket-029 documents
the end-of-period volume as current behaviour and (b) as an accepted, planned change, and ticket-030
says that `GeracaoMaxPontoMW` uses the end-of-period volume. Under the [ADD] path, ticket-029 documents
the mean volume as current behaviour, the S1 form and the ADR-006 symmetry, and ticket-030 documents
`GeracaoMaxPontoMW` at the period mean.

## Decision

PENDING

## Consequences

- Under (a): no code change; ticket-029 documents the end-of-period volume, the divergence from the Libs
  form and from ADR-006, and the `[to confirm]` items; the section 9 baseline stays current.
- Under (b) with [ADD]: a new implementation ticket changes the rows (S1), `GeracaoMaxPontoMW` (S2), the
  probe and the tests, and re-records the section 9 objectives; ticket-029 and ticket-030 document the
  period mean as current behaviour.
- Under (b) with a follow-up plan: ticket-029 documents the end-of-period volume as current behaviour and
  (b) as accepted and planned.
- Under either option, ADR-028 stays accepted (S3). The `[to confirm]` items stay open: the meaning of
  `V_i^t` in (9.33), the row form of DESSEM 22.0.2 under the Libs FPHA, the volume at which
  AVL_DESVFPHA evaluates `Ghid (FPHA`, and useful versus total volume in AVL_FPHA1 `Varm`. They need the
  MM/MU/Libs PDFs, the 032026 full case or a CEPEL answer.
