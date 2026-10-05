# Automatic Session Handoff / Runner Contract

## Important limitation

The actual creation of a new conversation depends on the agent runtime.
The project must not depend on an imaginary API.

The repository provides:
- persistent state;
- handoff;
- next action;
- machine-readable signals.

An external orchestrator can launch a new session.

## Handoff signal

```text
SESSION_HANDOFF_REQUIRED
```

Before this signal:
- state written;
- tests recorded;
- HANDOFF.md consistent;
- precise next action;
- no critical information kept only in chat.

## Continue signal

```text
SESSION_CAN_CONTINUE
```

Only if the task is small, tests are green, and no open blockers exist.

## Resumption prompt

```text
You are resuming an existing project.

Read AGENTS.md.
Read docs/agent/SESSION_STATE.md.
Read docs/agent/HANDOFF.md.
Read docs/agent/RULES.md.

Check git status and git diff.
Verify that the handoff matches the repository.
Replay relevant tests.
Do not rely on any memory from a previous session.
Resume only the indicated next action.

If the task becomes too large, perform a new handoff.
```

## External runner

A runner can:
1. launch the agent;
2. mount the same workspace;
3. inject `AGENTS.md`;
4. detect `SESSION_HANDOFF_REQUIRED`;
5. stop the session;
6. launch a new session on the same workspace;
7. preserve git and state files.

The runner must never reconstruct state from the old context.
