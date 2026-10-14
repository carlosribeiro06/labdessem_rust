# Iterative Workflow

This document describes the execution modes and solve stages in labdessem.

## Overview

Labdessem supports two execution modes controlled by `opcao_execucao` in `study_config.json`:

- **Mode 1 (Iterative)**: LP → MILP → LP-FIXED → LP-CALC-CMO with network cut iteration
- **Mode 2 (Single MILP)**: Direct MILP solve without iteration

The iterative mode is designed for network-constrained studies where DC flow constraints are added incrementally based on detected violations.

## Execution Modes

### Mode 1: Iterative (`opcao_execucao = 1`)

This mode runs a multi-stage workflow:

1. **LP relaxation** with iterative network cut addition
2. **MILP** for commitment decisions
3. **LP-FIXED** to verify feasibility with fixed commitments
4. **LP-CALC-CMO** for dual extraction (marginal costs)

When network treatment is disabled (`rede = 0`), the LP stage runs only once without iteration.

### Mode 2: Single MILP (`opcao_execucao = 2`)

Direct solve without iteration:

1. **MILP** with all network constraints (if enabled)

This mode is faster but may not find the optimal commitment sequence when network constraints are binding.

## Stage Details

### LP Stage

**Purpose**: Find an initial relaxed solution and identify network flow violations.

- Binary commitment variables relaxed to continuous [0, 1]
- Solves quickly (linear program)
- Output: relaxed generation/flow schedule, list of flow violations

**Network cut iteration**: If violations exist, add DC flow constraints for violated line-periods and re-solve. Repeat until no violations (or maximum iterations reached).

### MILP Stage

**Purpose**: Determine optimal commitment decisions.

- Binary commitment variables: thermal on/off, startup, shutdown
- Optional hydro commitment if `UCH = 1`
- Includes all network cuts accumulated during LP iteration

**Output**: Integer commitment schedule, generation dispatch.

### LP-FIXED Stage

**Purpose**: Verify the MILP solution respects all constraints with commitments fixed.

- Commitment variables fixed to MILP solution values
- Re-solves as LP for faster verification
- May detect infeasibilities not visible in MILP

### LP-CALC-CMO Stage

**Purpose**: Extract dual information for pricing.

- Same fixed commitments as LP-FIXED
- Extracts dual variables:
  - `PiDemanda`: Submarket marginal operating cost (CMO)
  - `PiBalHidr`: Water balance shadow prices

This stage produces the economic signals used in settlement and planning.

## Network Cut Iteration

When `rede = 1`, the workflow adds DC flow constraints iteratively:

1. Solve LP without network constraints
2. Compute DC flows using PTDF matrix
3. Identify lines where |flow| > limit
4. Add flow constraint for each violated (line, period) pair
5. Re-solve and repeat until convergence

**Flow constraint form**:

```
-limit ≤ Σ(PTDF[line,bus] × injection[bus]) ≤ limit
```

The injection at each bus is the sum of generation minus demand.

## Data Flow Between Stages

```
LP (iterations)
    │
    ├─ accumulated network cuts
    ▼
MILP
    │
    ├─ commitment decisions (on/off, startup, shutdown)
    ▼
LP-FIXED
    │
    ├─ verified dispatch
    ▼
LP-CALC-CMO
    │
    └─ dual variables (CMO, water values)
```

## Key Files

- [`crates/labdessem-simulation/src/workflow.rs`](../../crates/labdessem-simulation/src/workflow.rs) - `ExecutionOption` and `IterationStage` enums
- [`crates/labdessem-simulation/src/stages.rs`](../../crates/labdessem-simulation/src/stages.rs) - Stage orchestration and solve logic
- [`crates/labdessem-simulation/src/network/flow.rs`](../../crates/labdessem-simulation/src/network/flow.rs) - Flow violation detection
