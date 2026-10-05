# Expressive Control

Rust project for a generic expressive music controller.

Architecture:
`Sensors -> Controls -> Instrument -> Renderer -> MIDI/MPE -> Transport`

The Core is `no_std`, independent of hardware. The host PC and nRF5340
are separate platforms. YAML/Lua remain exclusive to the simulator.

See:
- `docs/architecture.md`
- `docs/roadmap.md`
- `docs/agent/RULES.md`
- `docs/agent/IMPLEMENTATION_PLAN.md`
- `docs/agent/PROMPTS.md`

Initial commands:
```bash
cargo test
cargo fmt --check
cargo run -p expressive-simulator
```
