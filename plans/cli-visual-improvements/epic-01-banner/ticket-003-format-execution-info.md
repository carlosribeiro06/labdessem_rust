# [TICKET-003] Format execution configuration block

## Context

### Background
After the banner, the CLI should display a formatted block showing the execution configuration: solver, mode, and active flags.

### Relation to Epic
Completes the visual identity by showing relevant execution info in a structured format.

### Current State
Configuration is shown as individual lines:
```
Execution option: MILP unico
Network constraints: disabled
Residual TON: disabled
...
```

## Specification

### Requirements
1. Create a function to format execution info as a structured block
2. Show: Solver, Execution mode, Active flags
3. Support colored and plain text variants
4. Integrate into the run flow (after banner, before "Reading input data...")

### Inputs/Props
```rust
pub struct ExecutionInfo {
    pub solver: &'static str,
    pub execution_mode: &str,
    pub network_enabled: bool,
    pub uct_enabled: bool,
    pub uch_enabled: bool,
    pub fpha_enabled: bool,
    pub fcf_enabled: bool,
    pub ton_residual_enabled: bool,
}
```

### Outputs/Behavior

**Colored output:**
```
Execution
  Solver:    HiGHS
  Mode:      MILP único
  Flags:     UCT
```

Or when multiple flags:
```
Execution
  Solver:    HiGHS
  Mode:      LP → MILP → LP-FIXED → LP-CALC-CMO
  Flags:     UCT, FCF, rede
```

**Plain output:**
Same content without ANSI color codes.

### Error Handling
N/A - pure string formatting

## Acceptance Criteria
- [ ] Execution block displays after banner
- [ ] Shows "HiGHS" as solver
- [ ] Shows execution mode description
- [ ] Shows active flags (only enabled ones)
- [ ] "Execution" header is bold when colored
- [ ] Falls back to plain text when not TTY
- [ ] Replaces current verbose flag output

## Implementation Guide

### Suggested Approach

1. Add to `banner.rs`:

```rust
pub struct ExecutionInfo<'a> {
    pub execution_mode: &'a str,
    pub network_enabled: bool,
    pub uct_enabled: bool,
    pub uch_enabled: bool,
    pub fpha_enabled: bool,
    pub fcf_enabled: bool,
    pub ton_residual_enabled: bool,
}

pub fn print_execution_info(info: &ExecutionInfo) {
    let colored = use_colors();
    let output = render_execution_info(info, colored);
    eprint!("{output}");
}

fn render_execution_info(info: &ExecutionInfo, use_color: bool) -> String {
    let mut flags = Vec::new();
    if info.uct_enabled { flags.push("UCT"); }
    if info.uch_enabled { flags.push("UCH"); }
    if info.fpha_enabled { flags.push("FPHA"); }
    if info.fcf_enabled { flags.push("FCF"); }
    if info.ton_residual_enabled { flags.push("TON_Residual"); }
    if info.network_enabled { flags.push("rede"); }
    
    let flags_str = if flags.is_empty() {
        "none".to_string()
    } else {
        flags.join(", ")
    };
    
    if use_color {
        format!(
            "\x1b[1mExecution\x1b[0m\n\
             \x1b[38;5;245m  Solver:    \x1b[0mHiGHS\n\
             \x1b[38;5;245m  Mode:      \x1b[0m{}\n\
             \x1b[38;5;245m  Flags:     \x1b[0m{}\n\n",
            info.execution_mode,
            flags_str
        )
    } else {
        format!(
            "Execution\n\
               Solver:    HiGHS\n\
               Mode:      {}\n\
               Flags:     {}\n\n",
            info.execution_mode,
            flags_str
        )
    }
}
```

2. Update `main.rs` to use the new functions:

```rust
use banner::{print_banner, print_execution_info, ExecutionInfo};

fn run_and_print(case_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    print_banner();
    
    // ... read config ...
    
    print_execution_info(&ExecutionInfo {
        execution_mode: execution_option.description(),
        network_enabled: config.rede != 0,
        uct_enabled: config.uct != 0,
        uch_enabled: config.uch != 0,
        fpha_enabled: config.fpha != 0,
        fcf_enabled: config.fcf != 0,
        ton_residual_enabled: config.ton_residual != 0,
    });
    
    // ... rest of the function ...
}
```

3. Remove the old individual println! statements for flags

### Key Files to Modify
- MODIFY: `crates/labdessem-cli/src/banner.rs` (add ExecutionInfo and print function)
- MODIFY: `crates/labdessem-cli/src/main.rs` (integrate new display, remove old output)

### Patterns to Follow
- Keep the "Execution" header consistent with cobre style
- Use dim/gray for labels, normal for values
- Only show enabled flags (don't list disabled ones)

### Pitfalls to Avoid
- Don't forget to remove the old verbose output
- Keep alignment consistent (use fixed-width labels)

## Testing Requirements

### Unit Tests
- Test `render_execution_info` with various flag combinations
- Test that only enabled flags appear
- Test colored vs plain output

### Integration Tests
- Manual: Run with different configs and verify output
- Manual: Verify output looks correct when piped

## Dependencies
- **Blocked By**: ticket-001, ticket-002
- **Blocks**: None (final ticket)

## Effort Estimate
**Points**: 2
**Confidence**: High
