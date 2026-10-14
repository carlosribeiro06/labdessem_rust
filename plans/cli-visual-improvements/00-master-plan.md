# Master Plan: CLI Visual Improvements

## Executive Summary

Add a professional visual identity to the labdessem CLI by displaying an ASCII art banner (based on the existing SVG logo), version info, and a formatted execution summary before running simulations. No new dependencies will be added.

## Goals & Non-Goals

### Goals
- Display ASCII art banner with logo, version, and tagline at startup
- Show execution configuration (solver, mode, active flags) in a formatted block
- Auto-detect TTY to disable colors when output is piped
- Maintain zero new dependencies

### Non-Goals
- Progress bars or spinners (current step-by-step output is sufficient)
- `--color` flag (auto-detect only)
- `--quiet` flag
- Interactive features

## Architecture Overview

### Current State
- CLI prints plain text messages during execution
- No visual branding or banner
- Configuration shown as simple key: value lines

### Target State
- CLI displays colored ASCII banner on startup (when TTY detected)
- Execution info shown in a formatted "Execution" block
- Falls back to plain text when stderr is not a TTY

### Key Design Decisions
1. **No new dependencies**: Use standard library for TTY detection (`std::io::IsTerminal`)
2. **Colors via ANSI codes**: Hardcode ANSI escape sequences directly
3. **Banner design**: Convert lightning bolt from SVG to ASCII art representation
4. **Modular structure**: Separate `banner.rs` module for display logic

## Technical Approach

### Tech Stack
- Rust standard library only
- ANSI escape codes for colors
- `std::io::IsTerminal` trait (stable since Rust 1.70)

### Component/Module Breakdown
```
crates/labdessem-cli/src/
├── main.rs          # Entry point, calls banner and run
├── banner.rs        # NEW: Banner rendering and TTY detection
└── ...
```

### Data Flow
1. CLI starts → detect if stderr is TTY
2. If TTY → print colored banner
3. If not TTY → print plain banner (no ANSI codes)
4. Print execution configuration block
5. Proceed with simulation (existing code)

### Testing Strategy
- Unit tests for banner string generation (colored and plain)
- Manual verification of visual output

## Phases & Milestones

| Phase | Epic | Description | Tickets |
|-------|------|-------------|---------|
| 1 | Banner & Display | ASCII banner, TTY detection, execution info | 3 |

**Total: 1 epic, 3 tickets**

## Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ASCII art doesn't look good | Medium | Low | Iterate on design, keep it simple |
| TTY detection edge cases | Low | Low | Fallback to plain text is safe |

## Success Metrics

- [ ] Banner displays correctly in terminal
- [ ] Banner falls back to plain text when piped
- [ ] Execution info shows all relevant configuration
- [ ] No new dependencies added
- [ ] All tests pass
