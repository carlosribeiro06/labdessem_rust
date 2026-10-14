# [TICKET-005] Implement run subcommand with case path argument

## Context

### Background
The CLI needs to be restructured to use a subcommand pattern (`labdessem run <path>`) and receive the case directory as an argument instead of reading it from the config file.

### Relation to Epic
This is the main behavioral change in Epic 02.

### Current State
```rust
// crates/labdessem-cli/src/main.rs
fn main() -> ExitCode {
    let config_path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../labdessem-io/study_config.json")
    });
    // ...
}
```

## Specification

### Requirements
1. Implement subcommand structure: `labdessem run [case_dir]`
2. If `case_dir` is provided, use it as the case directory
3. If `case_dir` is omitted, use current working directory
4. Read `study_config.json` from the case directory
5. Display professional error message if config not found
6. Show help text on `labdessem --help` and `labdessem run --help`

### Inputs/Props
- `case_dir`: Optional positional argument (defaults to `.`)

### Outputs/Behavior
```
$ labdessem run /path/to/case
Reading study configuration...
Case: /path/to/case
...

$ labdessem run
Reading study configuration...
Case: /current/working/directory
...

$ labdessem run /nonexistent
Error: Configuration file not found.
Expected: /nonexistent/study_config.json

Each study case must contain a study_config.json file in its root directory.
```

### Error Handling
- Missing config file: Clear message with expected path
- Invalid JSON: Parse error with file location
- Missing required fields: Validation error

## Acceptance Criteria
- [ ] `labdessem run /path/to/case` works correctly
- [ ] `labdessem run` uses current directory
- [ ] `labdessem --help` shows available commands
- [ ] `labdessem run --help` shows run command usage
- [ ] Missing config produces professional error message
- [ ] Exit code is non-zero on error

## Implementation Guide

### Suggested Approach

1. Parse command line arguments manually (no external crate needed for this simple case):
   ```rust
   fn main() -> ExitCode {
       let args: Vec<String> = env::args().collect();
       
       match args.get(1).map(|s| s.as_str()) {
           Some("run") => {
               let case_path = args.get(2)
                   .map(PathBuf::from)
                   .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
               run_simulation(&case_path)
           }
           Some("--help") | Some("-h") | None => {
               print_help();
               ExitCode::SUCCESS
           }
           Some(unknown) => {
               eprintln!("Unknown command: {unknown}");
               eprintln!("Run 'labdessem --help' for usage.");
               ExitCode::FAILURE
           }
       }
   }
   ```

2. Implement help text:
   ```rust
   fn print_help() {
       println!("labdessem - Hydrothermal dispatch optimization");
       println!();
       println!("USAGE:");
       println!("    labdessem <COMMAND>");
       println!();
       println!("COMMANDS:");
       println!("    run [CASE_DIR]    Run simulation for a study case");
       println!("                      If CASE_DIR is omitted, uses current directory");
       println!();
       println!("EXAMPLES:");
       println!("    labdessem run /path/to/case");
       println!("    labdessem run                   # uses current directory");
   }
   ```

3. Update `run_and_print` to accept case path:
   ```rust
   fn run_and_print(case_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
       let config_path = case_path.join("study_config.json");
       if !config_path.exists() {
           return Err(format!(
               "Configuration file not found.\n\
                Expected: {}\n\n\
                Each study case must contain a study_config.json file in its root directory.",
               config_path.display()
           ).into());
       }
       // ... rest of the function using case_path
   }
   ```

### Key Files to Modify
- `crates/labdessem/src/main.rs` (complete rewrite of argument handling)

### Patterns to Follow
- Keep it simple - no need for clap or other CLI crates
- Professional error messages
- Clear help text

### Pitfalls to Avoid
- Don't forget to canonicalize paths for display
- Handle `current_dir()` failure gracefully

## Testing Requirements

### Unit Tests
N/A - CLI behavior tested manually

### Integration Tests
Test from command line:
```bash
cargo run -p labdessem -- run examples/24Barras
cargo run -p labdessem -- run
cargo run -p labdessem -- --help
cargo run -p labdessem -- run --nonexistent  # should error
```

## Dependencies
- **Blocked By**: ticket-004, Epic 01
- **Blocks**: Epic 03

## Effort Estimate
**Points**: 3
**Confidence**: High
