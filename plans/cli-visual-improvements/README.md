# CLI Visual Improvements

## Summary

Add professional visual identity to the labdessem CLI with an ASCII art banner and formatted execution information.

## Mode

**Rigoroso** - English throughout, minimal comments.

## Epics

| Epic | Description | Tickets |
|------|-------------|---------|
| Epic 01 | Banner & Display | 3 |

**Total tickets: 3**

## Execution Order

```
ticket-001 (create banner module)
    ↓
ticket-002 (design ASCII banner)
    ↓
ticket-003 (format execution info)
```

## Quick Start

Execute the plan:
```
/implement-plan plans/cli-visual-improvements
```

Or execute individual tickets:
```
/implement plans/cli-visual-improvements/epic-01-banner/ticket-001-create-banner-module.md
```

## Expected Output

After implementation, running `labdessem run examples/24Barras` will show:

```
  ⚡ LABDESSEM v0.5.0
     Hydrothermal dispatch optimization

Execution
  Solver:    HiGHS
  Mode:      MILP único
  Flags:     UCT, FCF

Reading input data...
...
```
