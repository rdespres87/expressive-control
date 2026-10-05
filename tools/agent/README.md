# Agent Runner Integration

The external runner is optional.

If it exists, it must preserve the same workspace and relaunch the agent after
`SESSION_HANDOFF_REQUIRED`.

It must never invent or reconstruct the project state.
Resumption must go through the `docs/agent/*` files.
