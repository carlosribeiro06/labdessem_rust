# [001] Create ARCHITECTURE.md

## Context

Document the refactored workspace structure for onboarding and maintenance.

## Specification

Create `ARCHITECTURE.md` at workspace root with:
1. Executive summary of the workspace
2. Crate responsibilities (one paragraph each)
3. Dependency graph (ASCII or Mermaid)
4. Design principles
5. Key data flows

## Acceptance Criteria

- [ ] `ARCHITECTURE.md` exists at workspace root
- [ ] All 6 crates documented
- [ ] Dependency graph included
- [ ] Design principles listed

## Implementation Guide

See detailed template in the master plan. Key sections:

```markdown
# Architecture

## Crate Responsibilities

### Foundation
- **labdessem-core** — Domain model...

### Infrastructure
- **labdessem-solver** — Solver abstraction...

### I/O
- **labdessem-io** — Case reader and result writer...

### Algorithm
- **labdessem-model** — Optimization model construction...
- **labdessem-simulation** — End-to-end orchestration...

### Entry Point
- **labdessem-cli** — Command-line interface...

## Dependency Graph

## Design Principles
```

## Dependencies

- **Blocked By**: Epic 06
- **Blocks**: 002

## Effort Estimate

**Points**: 2
