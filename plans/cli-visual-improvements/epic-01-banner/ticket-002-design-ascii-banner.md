# [TICKET-002] Design and implement ASCII art banner

## Context

### Background
The labdessem logo is a stylized lightning bolt (see `docs/logo.svg`). We need to convert this into ASCII art that displays well in terminals.

### Relation to Epic
This is the main visual element of the CLI branding.

### Current State
Ticket-001 created a placeholder banner with just "LABDESSEM vX.Y.Z".

## Specification

### Requirements
1. Design ASCII art representation of the lightning bolt logo
2. Include version number and tagline
3. Support colored and plain text variants
4. Keep it compact (3-5 lines max)

### Inputs/Props
- `use_color: bool` - whether to emit ANSI escape codes

### Outputs/Behavior

**Colored output** (when TTY):
```
  ⚡ LABDESSEM v0.5.0
     Hydrothermal dispatch optimization
```

Or with box-drawing characters:
```
 ╭─────────────────────────────────────╮
 │  ⚡ LABDESSEM v0.5.0                │
 │     Hydrothermal dispatch           │
 ╰─────────────────────────────────────╯
```

**Plain output** (when piped):
Same content without ANSI color codes.

### Error Handling
N/A - pure string formatting

## Acceptance Criteria
- [ ] ASCII banner renders correctly in terminal
- [ ] Banner includes lightning bolt symbol (⚡ or ASCII representation)
- [ ] Banner includes version number
- [ ] Banner includes tagline "Hydrothermal dispatch optimization"
- [ ] Colored version uses cyan/blue tones (matching logo colors)
- [ ] Plain version has no ANSI escape codes
- [ ] Banner is max 5 lines tall

## Implementation Guide

### Suggested Approach

Update `render_banner()` in `banner.rs`:

```rust
fn render_banner(use_color: bool) -> String {
    let version = env!("CARGO_PKG_VERSION");
    
    if use_color {
        // Cyan bold for title, dim for tagline
        format!(
            "\n\
             \x1b[1;38;5;39m  ⚡ LABDESSEM\x1b[0m v{version}\n\
             \x1b[38;5;245m     Hydrothermal dispatch optimization\x1b[0m\n\n"
        )
    } else {
        format!(
            "\n\
               ⚡ LABDESSEM v{version}\n\
                  Hydrothermal dispatch optimization\n\n"
        )
    }
}
```

**Color codes reference:**
- `\x1b[1;38;5;39m` - Bold + bright cyan (256-color)
- `\x1b[38;5;245m` - Gray (256-color)
- `\x1b[0m` - Reset

**Alternative with box drawing:**
```rust
fn render_banner(use_color: bool) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let title = format!("⚡ LABDESSEM v{version}");
    let tagline = "Hydrothermal dispatch optimization";
    
    if use_color {
        format!(
            "\n\
             \x1b[38;5;39m  {title}\x1b[0m\n\
             \x1b[38;5;245m     {tagline}\x1b[0m\n\n"
        )
    } else {
        format!(
            "\n\
               {title}\n\
                  {tagline}\n\n"
        )
    }
}
```

### Key Files to Modify
- MODIFY: `crates/labdessem-cli/src/banner.rs`

### Patterns to Follow
- Use 256-color ANSI codes for better terminal compatibility
- Keep banner compact and professional
- Match the blue/cyan color scheme of the SVG logo

### Pitfalls to Avoid
- Don't make the banner too tall (wastes screen space)
- Don't use complex ASCII art that breaks with different fonts
- Test with both light and dark terminal themes

## Testing Requirements

### Unit Tests
- Update existing tests to verify new banner content
- Test that ⚡ symbol is present
- Test that version is present
- Test that tagline is present

### Integration Tests
- Manual: Visual inspection in terminal
- Manual: Verify colors look good on dark and light backgrounds

## Dependencies
- **Blocked By**: ticket-001
- **Blocks**: ticket-003

## Effort Estimate
**Points**: 2
**Confidence**: Medium (design iteration may be needed)
