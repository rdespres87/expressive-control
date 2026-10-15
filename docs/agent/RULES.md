# Project Rules

## 1. Language Rules
- **English-Only Repository:** ALL repository contents MUST be authored exclusively in English without exception:
  - **Documentation & Studies:** ALL markdown files (`*.md`), architecture studies (`docs/studies/`), design specifications, and agent state files MUST be written in English. Even when the user converses in French, NEVER write repository documentation in French.
  - **Code & Comments:** ALL code, rustdoc (`///`), and inline comments (`//`) MUST be written in English.
  - **Commit Messages:** ALL commit messages and PR descriptions MUST be written in English.


## 2. Git & Version Control
- **Conventional Commits:** ALL commit messages MUST follow the Conventional Commits specification.
- **No Remote Push Without Explicit Approval (STRICT):** You MUST NEVER execute `git push` to remote repositories without explicit, direct user approval in the chat. Local commits are permitted once tests and pre-commit checks pass, but pushing to remote must wait until the user has tested or explicitly commanded to push.
- **State Persistence:** Agent management files (in `docs/agent/` and `AGENTS.md`) ARE part of the repository and MUST be committed.
- **Pre-commit Checks (MANDATORY):** Before ANY commit, you MUST run `cargo fmt --all` and `cargo clippy --workspace -- -D warnings`. No warnings or formatting errors are allowed. You CANNOT commit if there are any warnings.
- **Git Hook:** A `.git/hooks/pre-commit` script is installed and active. It will automatically reject any commit if formatting or clippy fails. Always fix them before committing.
- **Synchronized Commits:** You MUST ensure that any changes to agent tracking files (`TASKS.md`, `SESSION_STATE.md`, `HANDOFF.md`) are committed together with the code changes they represent. NEVER declare a task finished or emit `SESSION_HANDOFF_REQUIRED` if tracking files have uncommitted modifications.
- **README Maintenance:** You MUST proactively update the `README.md` file whenever a new architectural component (e.g., a new workspace member, new instrument, or new firmware) is added or when major features are completed.

## 3. Architecture Rules
- **Core `no_std`:** The `expressive-core` crate MUST remain strictly `#![no_std]`. No hardware-specific dependencies are allowed in this crate.
- **Hardware Isolation:** All hardware drivers and specific behaviors must be implemented in the respective firmware crates, not in the core.

## 4. Coding Standards
- **No Panics in Production:** The use of `unwrap()`, `expect()`, or `panic!()` is strictly FORBIDDEN in production code. All errors must be handled properly using `Result`, custom error enums, and graceful degradation/logging.
- **High Test Coverage:** For every new implementation or feature, you MUST add the corresponding unit tests AND update the integration tests accordingly. Maintaining a very high test coverage across both unit and integration tests is mandatory to ensure reliability.
- **Mandatory Post-Implementation Grill:** Always execute a final adversarial code review pass ("Grill") right after implementation and before Gate 3 commit authorization. Actively probe for edge cases, subtle logic inversions (e.g. mute vs volume, polarity, inverted ranges), concurrency race conditions / queue saturations, and add missing unit tests accordingly.

