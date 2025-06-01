# Guidance for Automated Contributors

- **Do not edit files under `examples/` manually.** These are generated from templates in `templates/` using the code in `src/generator/`.
- Update the templates or generator, then run the generator command as described in `CONTRIBUTING.md`.
- Use `cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force` (or `just gen`) to regenerate the example project.
- Run `cargo fmt` and `cargo test` before committing changes.
- See `CONTRIBUTING.md` for a full description of the repository layout and workflow.
- After regeneration, run:
```bash
cargo fmt
cargo test -- --nocapture
```
