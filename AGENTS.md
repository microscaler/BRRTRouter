# Codex Agent Instructions

- **Do not manually edit files in `examples/`.** Modify templates in `templates/` and regenerate.
- Use `cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force` (or `just gen`) to regenerate the example project.
- See `CONTRIBUTING.md` for detailed steps.
- After regeneration, run:
  ```bash
  cargo fmt
  cargo test -- --nocapture
  ```
