# Agent Bootstrap

## First read
1. `AGENTS.md`
2. `docs/agent/SESSION_PROTOCOL.md`
3. `docs/agent/SESSION_STATE.md`
4. `docs/agent/HANDOFF.md`
5. `docs/agent/RULES.md`
6. `docs/agent/CONTRACTS.md`
7. `docs/agent/TASK_DECOMPOSITION.md`
8. `docs/agent/DEFINITION_OF_DONE.md`
9. `docs/agent/TASKS.md`
10. `docs/agent/IMPLEMENTATION_PLAN.md`

## Inspect
- `git status`
- `git diff`
- Cargo workspace
- crates
- tests

## Report before editing
- build status;
- test status;
- existing crates;
- architecture violations;
- next task;
- planned files;
- risks.

Do not modify anything before this diagnosis.

## Core rules
- Core `no_std`.
- no hardware dependencies in Core.
- YAML/Lua only in simulator.
- InstrumentEvent does not depend on MIDI.
- drivers produce SensorEvent.
- transport separated from renderer.
- no unverified external dependencies.

## Session rule
If the task cannot be cleanly completed within the session:
perform a handoff rather than leaving an ambiguous state.
