# Contributing

Thank you for helping improve **BRRTRouter**! The example application in
`examples/pet_store` is automatically generated from
`examples/openapi.yaml`.
The generator logic lives in `src/generator` and uses templates from
`templates/`.

## Updating the generated examples

1. Update the templates or generator code.
2. Run the generator:

   ```bash
   cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force
   ```
   *(or run `just gen`)*
3. Commit any regenerated files as part of your change.

Direct edits to files inside `examples/pet_store` will be overwritten the
next time the generator runs.

