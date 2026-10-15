# Orchestration & Security Gates (Inviolable Rules)

## Mandatory Grill Orchestration
- For all non-trivial engineering tasks (architecture changes, memory optimizations, UI overhauls, submenus, protocol changes, drivers), you MUST automatically execute the `grill-orchestrator` workflow:
  1. **Gate 1 (Plan Approval):** Formulate the architecture draft as a markdown Artifact with `RequestFeedback: true`. Wait for user approval before modifying code.
  2. **Adversarial Inquisitor Review:** Audit code for memory bloat, hardware decoupling, `no_std` constraints, and silent errors.
  3. **Post-Implementation Grill & Tests:** Probe for edge cases, race conditions, and boundary inversions; add targeted unit tests.
  4. **Gate 3 (Commit Approval):** Display exact `git diff` and summary in chat, and explicitly ask the user for authorization before running `git commit`.
  5. **No Remote Push Without Explicit Approval:** Local commits only; never push to remote without explicit user command in chat.
