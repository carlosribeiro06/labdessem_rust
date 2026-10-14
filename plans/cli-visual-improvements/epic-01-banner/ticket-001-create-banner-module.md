# [TICKET-001] Create banner module with TTY detection

## Context

### Background
The CLI needs a module to handle visual output: detecting whether stderr is a TTY (to enable/disable colors) and rendering the banner.

### Relation to Epic
Foundation for all visual improvements. Other tickets depend on this module existing.

### Current State
No banner module exists. All output goes directly to stdout/stderr via `println!`.

## Specification

### Requirements
1. Create `crates/labdessem-cli/src/banner.rs` module
2. Implement TTY detection using `std::io::IsTerminal`
3. Provide function to check if colors should be used
4. Provide basic structure for banner rendering (actual design in ticket-002)

### Inputs/Props
- None (uses stderr for TTY detection)

### Outputs/Behavior
```rust
// Public API
pub fn use_colors() -> bool;
pub fn print_banner();
```

### Error Handling
- TTY detection should never panic; fallback to no colors on any error

## Acceptance Criteria
- [ ] `banner.rs` module created
- [ ] `use_colors()` returns `true` when stderr is a TTY
- [ ] `use_colors()` returns `false` when stderr is piped
- [ ] Module compiles without new dependencies
- [ ] Module is declared in `main.rs` with `mod banner;`

## Implementation Guide

### Suggested Approach

1. Create the file `crates/labdessem-cli/src/banner.rs`:

```rust
//! Terminal banner and display utilities for the labdessem CLI.

use std::io::{IsTerminal, stderr};

/// Check if colored output should be used.
/// Returns true when stderr is connected to a TTY.
pub fn use_colors() -> bool {
    stderr().is_terminal()
}

/// Print the labdessem banner to stderr.
pub fn print_banner() {
    let colored = use_colors();
    let banner = render_banner(colored);
    eprint!("{banner}");
}

fn render_banner(use_color: bool) -> String {
    // Placeholder - will be implemented in ticket-002
    let version = env!("CARGO_PKG_VERSION");
    if use_color {
        format!("\x1b[1;36mLABDESSEM v{version}\x1b[0m\n")
    } else {
        format!("LABDESSEM v{version}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::render_banner;

    #[test]
    fn render_banner_plain_contains_version() {
        let banner = render_banner(false);
        assert!(banner.contains("LABDESSEM v"));
        assert!(!banner.contains("\x1b["));
    }

    #[test]
    fn render_banner_colored_contains_ansi() {
        let banner = render_banner(true);
        assert!(banner.contains("\x1b["));
        assert!(banner.contains("\x1b[0m"));
    }
}
```

2. Add `mod banner;` to `main.rs`

### Key Files to Modify
- CREATE: `crates/labdessem-cli/src/banner.rs`
- MODIFY: `crates/labdessem-cli/src/main.rs` (add `mod banner;`)

### Patterns to Follow
- Use `eprint!` / `eprintln!` for banner output (stderr, not stdout)
- Keep public API minimal
- Separate rendering logic from printing

### Pitfalls to Avoid
- Don't use `println!` for banner (stdout is for data, stderr for UI)
- Don't add any dependencies

## Testing Requirements

### Unit Tests
- Test that `render_banner(false)` produces no ANSI codes
- Test that `render_banner(true)` produces ANSI codes
- Test that version string is present

### Integration Tests
- Manual: Run `labdessem run examples/24Barras` and verify banner appears
- Manual: Run `labdessem run examples/24Barras 2>/dev/null` and verify no banner in terminal

## Dependencies
- **Blocked By**: None
- **Blocks**: ticket-002, ticket-003

## Effort Estimate
**Points**: 1
**Confidence**: High
