# Epic 01: Banner & Display

## Overview

Implement the visual identity for the labdessem CLI: ASCII art banner with logo, version, tagline, and formatted execution information.

## Scope

- Create `banner.rs` module with TTY detection and banner rendering
- Design ASCII art based on the lightning bolt SVG logo
- Format execution configuration as a structured block
- Integrate into main.rs

## Tickets

1. `ticket-001-create-banner-module.md` - Create banner module with TTY detection
2. `ticket-002-design-ascii-banner.md` - Design and implement ASCII art banner
3. `ticket-003-format-execution-info.md` - Format execution configuration block

## Dependencies

- None (first epic)

## Acceptance Criteria

- [ ] Banner displays on CLI startup
- [ ] Colors disabled when stderr is not a TTY
- [ ] Execution info shows solver, mode, and active flags
- [ ] No new dependencies added to Cargo.toml
