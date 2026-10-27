# Design Documents

Architectural decisions and domain knowledge for labdessem.

## Documents

| Document | Description | Status |
|----------|-------------|--------|
| [Iterative Workflow](iterative-workflow.md) | Execution modes and solve stages | ✅ |
| [FPHA Formulation](fpha-formulation.md) | Hydro generation approximation | ✅ |
| [EngMax Formulation](engmax-formulation.md) | Dynamic maximum swallowing | ✅ |
| [Future Cost Function](future-cost-function.md) | Benders cuts and FCF binary format | ✅ |
| [Network Cuts](network-cuts.md) | DC flow calculation and cut generation | ✅ |
| [Hydro Modeling](hydro-modeling.md) | Cascade topology, pumping, and diversion | ✅ |
| [CMB/CMO Formulation](cmb-cmo-formulation.md) | Nodal and submarket marginal costs | ✅ |

## Purpose

These documents capture:

1. **Architectural decisions** — why the code is structured as it is
2. **Domain knowledge** — power system concepts and mathematical formulations
3. **Implementation rationale** — trade-offs made during development

## Structure

Each document follows this template:

```markdown
# Title

Brief description of what this document covers.

## Overview
High-level explanation of the concept.

## Implementation
How it's implemented in labdessem.

## Key Files
Links to relevant source files.

## References
External resources, papers, or standards.
```

## Contributing

When adding a new design document:

1. Add entry to this README table
2. Use consistent structure (Overview, Implementation, Key Files)
3. Link to relevant source code
4. Keep documents focused (~1-2 pages)
