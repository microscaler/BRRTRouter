# Documentation Assets

This directory contains assets for enhancing the generated rustdoc documentation.

## Files

### `head.html`

Custom HTML that is injected into the `<head>` of every rustdoc page. This file loads Mermaid.js to enable rendering of Mermaid diagrams in the documentation.

**What it does:**
- Loads Mermaid.js v10 from CDN
- Configures Mermaid with appropriate settings
- Automatically renders all code blocks with the `mermaid` language tag as interactive diagrams

**Usage:**

The `head.html` file is automatically included when building documentation via:

1. **`.cargo/config.toml`** - Configured for local development
2. **`Cargo.toml`** - Configured via `[package.metadata.docs.rs]` for docs.rs builds

You don't need to do anything special - just run:

```bash
cargo doc --no-deps --lib --open
```

## Mermaid Diagrams in Docs

To include a Mermaid diagram in your Rust documentation:

```rust
//! ```mermaid
//! sequenceDiagram
//!     participant A
//!     participant B
//!     A->>B: Hello!
//!     B-->>A: Hi there!
//! ```
```

The diagram will automatically render as an interactive SVG in the generated documentation.

## Supported Diagram Types

Mermaid supports many diagram types:

- Sequence diagrams
- Flowcharts
- Class diagrams
- State diagrams
- Entity-relationship diagrams
- Gantt charts
- Git graphs
- And more!

See the [Mermaid documentation](https://mermaid.js.org/) for details.

## Maintenance

If you need to update the Mermaid version or configuration:

1. Edit `doc/head.html`
2. Rebuild docs: `cargo doc --no-deps --lib`
3. Test that diagrams render correctly in your browser

## Technical Details

**Why this approach?**

Rustdoc doesn't natively render Mermaid diagrams - it just shows them as code blocks. By injecting Mermaid.js via the HTML header, we enable client-side rendering of diagrams when the documentation is viewed in a browser.

**Alternative approaches:**

1. Generate SVG files from Mermaid source and embed them as images (requires build-time processing)
2. Use a rustdoc plugin (not widely supported yet)
3. Host diagrams externally (not self-contained)

The `--html-in-header` approach is the simplest and most maintainable solution.

