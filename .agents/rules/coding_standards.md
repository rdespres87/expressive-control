# Coding Standards: Inviolable Rules

## Zero `unwrap()` or `expect()` in Production Code
- **Inviolable Requirement:** Production code (`core/`, `instruments/`, `firmwares/`) MUST contain exactly **0** instances of `.unwrap()`, `.expect()`, or `panic!()`.
- All errors, `None`, or failure states must be handled using:
  - Rust `Result` / `Option` matching or propagation (`?`).
  - Safe fallback functions (`unwrap_or`, `unwrap_or_else`, `unwrap_or_default`).
  - Safe pattern matching (`if let Ok(...) = ...` / `if let Some(...) = ...`).
- **Mutex Guards:** Never write `.lock().unwrap()`. Always use `if let Ok(mut guard) = mutex.lock()` or `.lock().unwrap_or_else(|e| e.into_inner())` to prevent thread panics on poisoned locks.
- **Exceptions:** `.unwrap()` is tolerated ONLY in unit tests (`#[cfg(test)] mod tests`) and desktop simulator scenario YAML loading.
