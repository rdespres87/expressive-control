# Session Protocol

## Principle

The repository is the project's persistent memory. Any information
required for a future session must be written to the repository.

## Mandatory cycle

```text
START
 -> READ STATE
 -> VERIFY REPOSITORY
 -> SELECT ONE TASK
 -> PLAN
 -> IMPLEMENT
 -> TEST
 -> UPDATE STATE
 -> HANDOFF OR CONTINUE
```

At startup:
1. read `AGENTS.md`;
2. read `SESSION_STATE.md`;
3. read `HANDOFF.md` if it exists;
4. read `RULES.md`;
5. read relevant contracts;
6. run `git status` and `git diff`;
7. run baseline tests prior to modification.

## One session = one work unit

A session targets a coherent, testable, and verifiable unit.

Examples:
- implement clamp;
- implement a filter;
- add MPE allocator tests;
- add a minimal sensor driver.

Too large:
- all of `ControlProcessor`;
- the entire simulator;
- entire nRF5340 integration.

## Splitting triggers

Split if a task:
- touches more than two architectural boundaries;
- requires multiple independent decisions;
- exceeds roughly 10 files;
- exceeds roughly 300 net lines without justification;
- involves multiple independent technical risks;
- requires external research.

## Mandatory handoff

Perform a handoff if:
- context becomes insufficient;
- an architectural decision emerges;
- an external dependency must be investigated;
- multiple independent subtasks emerge;
- continuing increases the risk of losing track of work state.

Before stopping:
- update `SESSION_STATE.md`;
- write `HANDOFF.md`;
- update `TASKS.md`;
- record test runs;
- record modified files;
- define ONE precise next action.

## Resumption

The new session:
1. reads `AGENTS.md`, `SESSION_STATE.md`, `HANDOFF.md`;
2. verifies `git status` and `git diff`;
3. verifies that the handoff matches the repository;
4. replays relevant tests;
5. resumes only the specified next action.

Never reconstruct state from chat memory.

## Auto-session

Signal for an external orchestrator:

`SESSION_HANDOFF_REQUIRED`

Signal permitting continuation:

`SESSION_CAN_CONTINUE`

Never claim to have created a new session if the runtime does not actually
provide this capability. The repository provides continuity; an external
runner provides orchestration.
