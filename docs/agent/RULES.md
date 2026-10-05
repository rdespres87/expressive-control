# Project Rules

## 1. Language Rules
- **Code Comments:** ALL code comments (including rustdoc `///` and inline `//`) MUST be written in English.
- **Documentation:** ALL markdown files, architecture documents, and agent state files MUST be written in English.
- **Commit Messages:** ALL commit messages MUST be written in English.

## 2. Git & Version Control
- **Conventional Commits:** ALL commit messages MUST follow the Conventional Commits specification (e.g., `feat: ...`, `fix: ...`, `chore: ...`, `refactor: ...`, `docs: ...`).
- **State Persistence:** Agent management files (in `docs/agent/` and `AGENTS.md`) ARE part of the repository and MUST be committed. They act as the persistent memory of the project.

## 3. Architecture Rules
- **Core `no_std`:** The `expressive-core` crate MUST remain strictly `#![no_std]`. No hardware-specific dependencies are allowed in this crate.
- **Hardware Isolation:** All hardware drivers and specific behaviors must be implemented in the respective firmware crates (e.g., `firmwares/nrf5340`), not in the core.
