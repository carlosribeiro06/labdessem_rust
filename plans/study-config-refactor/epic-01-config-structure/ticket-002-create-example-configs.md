# [TICKET-002] Create study_config.json in example cases

## Context

### Background
Each example case needs its own `study_config.json` file to be self-contained. Currently, there's a single config file in `crates/labdessem-io/`.

### Relation to Epic
This ticket provides the test data needed for the new structure and removes the old config file.

### Current State
```
crates/labdessem-io/study_config.json  ← exists, to be deleted
examples/24Barras/                      ← no config
examples/3Barras/                       ← no config
examples/caso_sin_sem_rede/             ← no config
```

## Specification

### Requirements
1. Create `study_config.json` in each of the 3 example case directories
2. Delete `crates/labdessem-io/study_config.json`
3. All configs should have identical content (as specified by user):
   ```json
   {
     "opcao_execucao": 2,
     "rede": 0,
     "UCT": 1,
     "UCH": 0,
     "TON_Residual": 0,
     "FPHA": 0,
     "FCF": 1
   }
   ```

### Inputs/Props
N/A - file creation task

### Outputs/Behavior
Three new files created, one old file deleted.

### Error Handling
N/A

## Acceptance Criteria
- [ ] `examples/24Barras/study_config.json` exists with correct content
- [ ] `examples/3Barras/study_config.json` exists with correct content
- [ ] `examples/caso_sin_sem_rede/study_config.json` exists with correct content
- [ ] `crates/labdessem-io/study_config.json` is deleted
- [ ] JSON files are valid and parseable

## Implementation Guide

### Suggested Approach
1. Create the JSON content (identical for all 3 cases)
2. Write to each example directory
3. Delete the old config file
4. Verify with `cargo check`

### Key Files to Modify
- CREATE: `examples/24Barras/study_config.json`
- CREATE: `examples/3Barras/study_config.json`
- CREATE: `examples/caso_sin_sem_rede/study_config.json`
- DELETE: `crates/labdessem-io/study_config.json`

### Patterns to Follow
- Use consistent JSON formatting (2-space indent)
- Keep field order consistent with existing convention

### Pitfalls to Avoid
- Ensure FCF=1 only for cases that have OPER_FCF.bin (24Barras has it, others may not)

**Note**: The user specified FCF=1 for all cases. If a case doesn't have OPER_FCF.bin, set FCF=0 for that case to avoid runtime errors.

## Testing Requirements

### Unit Tests
N/A - file creation

### Integration Tests
Verify files exist and are valid JSON after creation.

## Dependencies
- **Blocked By**: ticket-001 (struct must be updated first)
- **Blocks**: ticket-003

## Effort Estimate
**Points**: 1
**Confidence**: High
