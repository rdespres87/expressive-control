# Git Push Policy

- **No Remote Push Without Explicit User Approval (Strict):** You MUST NEVER run `git push` to remote repositories without explicit, direct user approval in the chat.
- Commits may be created locally once all tests and pre-commit checks (`cargo fmt`, `cargo clippy`) pass cleanly, but pushing to the remote repository is strictly forbidden until the user has tested locally and explicitly asked to push.
