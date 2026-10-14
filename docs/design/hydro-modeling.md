# Hydro Modeling

This document describes the hydro plant modeling in labdessem, including cascade topology, pumping plants, and water diversion.

## Overview

Hydro plants are the most complex entities in labdessem due to:

1. **Cascade topology** - Water flows downstream from plant to plant
2. **Pumping plants** - Can pump water back upstream
3. **Water diversion** - Can divert water to non-downstream plants
4. **Multi-unit structure** - Plants have groups and units with different characteristics
5. **FPHA formulation** - Nonlinear generation approximation (see [fpha-formulation.md](fpha-formulation.md))

## Cascade Topology

### Structure

Each hydro plant can have:

- `upstream_plant_ids` - Plants whose outflow enters this plant's reservoir
- `downstream_plant_id` - Plant that receives this plant's outflow
- `diversion_upstream_plant_ids` - Plants whose diversion enters this plant
- `diversion_plant_id` - Plant that receives this plant's diverted water

### Hydraulic Balance

The water balance at each plant and period is:

```
volume[t+1] = volume[t]
            + inflow[t]
            + Σ(upstream turbining + spillage)
            + Σ(diversion upstream diverted water)
            - turbining
            - spillage
            - diversion
            + pumped water (if pumping plant exists)
```

All flow quantities are converted to hm³ using:
```
volume_hm3 = flow_m3s × period_hours × 0.0036
```

### Validation

The cascade topology is validated during system construction:

1. No circular references (plant cannot be its own upstream/downstream)
2. Bidirectional consistency (if A is upstream of B, B is downstream of A)
3. All referenced plant IDs must exist

## Pumping Plants

### Configuration

Pumping plants are defined in `CAD_USIE.csv` with:

- Source reservoir (where water is pumped from)
- Target hydro plant (where water arrives)
- Pump capacity (m³/s)
- Pumping efficiency (ratio)
- Electrical consumption (MW)

### Modeling

The pumping plant adds:

1. **Pumping variable** - Water volume pumped per period
2. **Electrical load** - Consumption added to submarket demand
3. **Water balance terms** - Pumped water enters target reservoir

```
target_volume[t+1] += pumped_water[t]
submarket_demand[t] += pump_consumption[t]
```

## Water Diversion

### Purpose

Water diversion allows a plant to send water to a plant that is not its natural downstream neighbor. This is used for:

- Canal systems connecting non-adjacent reservoirs
- Irrigation offtakes
- Environmental flow requirements

### Modeling

The diversion variable represents water diverted from a plant's outflow:

```
downstream_receives = turbining + spillage - diversion
diversion_target_receives += diversion
```

Diversion is bounded by the plant's diversion capacity.

## Unit and Group Structure

### Hierarchy

```
HydroPlant
├── Reservoir
│   ├── min_volume_hm3
│   ├── max_volume_hm3
│   └── initial_volume_hm3
├── Groups
│   └── HydroGroup
│       ├── name
│       └── productivity (MW·s/m³)
└── Units
    └── HydroUnit
        ├── name
        ├── group_idx
        ├── min_turbining_m3s
        └── max_turbining_m3s
```

### Aggregation

Variables are defined at different levels:

- **Volume, spillage, diversion**: Per plant
- **Turbining, generation**: Per unit
- **Commitment** (if UCH=1): Per unit

## Initial Conditions

Each plant has initial conditions:

- `initial_volume_hm3` - Starting reservoir volume
- `initial_turbining_m3s` - Turbining at t=0 (for ramp constraints)
- `initial_spillage_m3s` - Spillage at t=0

## Key Constraints

| Constraint | Level | Description |
|------------|-------|-------------|
| Hydraulic balance | Plant | Conservation of water |
| Turbining limits | Unit | Min/max turbining bounds |
| Generation limits | Unit | Min/max generation bounds |
| Volume limits | Plant | Reservoir capacity |
| Spillage limits | Plant | Spillway capacity |
| FPHA cuts | Plant | Generation envelope |

## Key Files

- [`crates/labdessem-core/src/entities/hydro.rs`](../../crates/labdessem-core/src/entities/hydro.rs) - `HydroPlant`, `HydroGroup`, `HydroUnit` structs
- [`crates/labdessem-core/src/system/mod.rs`](../../crates/labdessem-core/src/system/mod.rs) - Cascade validation
- [`crates/labdessem-model/src/constraints/hydro.rs`](../../crates/labdessem-model/src/constraints/hydro.rs) - Hydro constraints
- [`crates/labdessem-io/src/readers/hydro.rs`](../../crates/labdessem-io/src/readers/hydro.rs) - Hydro data readers
