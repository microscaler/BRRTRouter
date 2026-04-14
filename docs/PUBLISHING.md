# Publishing Guide

This guide covers how to publish BRRTRouter to crates.io and ensure documentation is properly published to docs.rs and GitHub Pages.

## Prerequisites

1. **Crates.io Account**: Create an account at https://crates.io
2. **API Token**: Generate an API token from https://crates.io/me
3. **GitHub Pages**: Ensure GitHub Pages is enabled for the repository

## Documentation Publishing

BRRTRouter documentation is published to three locations:

1. **docs.rs** - Automatic when publishing to crates.io
2. **GitHub Pages** - Automatic on push to main
3. **Local** - Available via `just docs`

### 1. Publishing to docs.rs (via crates.io)

Documentation is automatically built and published to docs.rs when you publish a new version to crates.io.

#### Configuration

The `Cargo.toml` includes docs.rs configuration:

```toml
[package.metadata.docs.rs]
rustdoc-args = ["--html-in-header", "doc/head.html"]
all-features = true
default-target = "x86_64-unknown-linux-gnu"
```

This ensures:
- ✅ Mermaid diagrams render correctly
- ✅ All features are documented
- ✅ Consistent build target

#### Publishing Process

```bash
# 1. Update version in Cargo.toml
vim Cargo.toml  # Bump version number

# 2. Update CHANGELOG.md
vim CHANGELOG.md  # Document changes

# 3. Verify documentation builds locally
just docs-check

# 4. Build and test everything
cargo build --release
cargo test --all

# 5. Commit changes
git add .
git commit -m "Bump version to X.Y.Z"
git tag vX.Y.Z
git push origin main --tags

# 6. Publish to crates.io
cargo publish
```

#### Verification

After publishing, verify documentation at:
- https://docs.rs/brrtrouter/latest/brrtrouter/

Documentation typically appears within 5-10 minutes of publishing.

### 2. GitHub Pages Documentation

Documentation is automatically deployed to GitHub Pages on every push to `main`.

#### Workflow

The `.github/workflows/docs.yml` workflow:
1. Builds documentation with Mermaid support
2. Creates index.html redirect
3. Deploys to GitHub Pages

#### Access

Documentation is available at:
- https://microscaler.github.io/BRRTRouter/

#### Manual Trigger

You can manually trigger documentation deployment:

```bash
# Via GitHub UI
# Go to Actions → Documentation → Run workflow

# Or push to main
git push origin main
```

### 3. Local Documentation

Generate documentation locally for development:

```bash
# Generate and open
just docs

# Generate only
just docs-build

# Check for warnings
just docs-check
```

## Pre-Publishing Checklist

Before publishing a new version:

### Code Quality

- [ ] All tests pass: `cargo test --all`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt --check`
- [ ] Documentation builds: `just docs-check`
- [ ] Examples work: `just start-petstore` and test endpoints

### Documentation

- [ ] All public APIs documented
- [ ] Examples updated if APIs changed
- [ ] CHANGELOG.md updated with changes
- [ ] README.md reflects current state
- [ ] Architecture diagrams up to date

### Version Management

- [ ] Version bumped in `Cargo.toml`
- [ ] Version follows SemVer:
  - Patch (0.1.X): Bug fixes, documentation
  - Minor (0.X.0): New features, backwards compatible
  - Major (X.0.0): Breaking changes
- [ ] Git tag created: `git tag vX.Y.Z`

### Legal

- [ ] LICENSE files present (MIT and Apache-2.0)
- [ ] All dependencies have compatible licenses
- [ ] Copyright headers up to date

## First-Time Publishing

For the initial crates.io release:

### 1. Configure Cargo

```bash
# Login to crates.io
cargo login YOUR_API_TOKEN
```

### 2. Verify Package

```bash
# Dry run to check what will be published
cargo publish --dry-run

# Review package contents
cargo package --list
```

### 3. Publish

```bash
# Publish to crates.io
cargo publish

# If you need to yank a version (emergency only)
# cargo yank --vers 0.1.0
```

## Documentation Features

BRRTRouter documentation includes several special features:

### Mermaid Diagrams

The documentation includes interactive Mermaid sequence diagrams for:
- Code generation flow
- Request handling flow

These are rendered via `doc/head.html` which loads Mermaid.js.

### Example Project

The Pet Store example is fully documented with:
- Project structure
- Running instructions
- API examples
- Configuration guide

### Telemetry Guide

Comprehensive observability documentation covering:
- Prometheus metrics
- OpenTelemetry tracing
- Health checks
- Structured logging

## Troubleshooting

### Docs.rs Build Fails

If docs.rs build fails:

1. Check build logs at https://docs.rs/crate/brrtrouter/*/builds
2. Verify `doc/head.html` is included in the package:
   ```bash
   cargo package --list | grep doc/head.html
   ```
3. Test locally with docs.rs environment:
   ```bash
   RUSTDOCFLAGS="--html-in-header doc/head.html" cargo doc --no-deps
   ```

### Mermaid Diagrams Not Rendering

If diagrams don't render on docs.rs:

1. Verify `doc/head.html` is in the published package
2. Check browser console for JavaScript errors
3. Ensure Mermaid CDN is accessible
4. Test locally first: `just docs`

### GitHub Pages Not Updating

If GitHub Pages doesn't update:

1. Check workflow runs: https://github.com/microscaler/BRRTRouter/actions
2. Verify Pages is enabled: Settings → Pages
3. Check that workflow has Pages permissions
4. Manually trigger workflow: Actions → Documentation → Run workflow

## Post-Publishing

After publishing a new version:

### 1. Announce

- [ ] Post to Reddit (r/rust)
- [ ] Post to Twitter/X
- [ ] Update GitHub release with changelog
- [ ] Email announcement list (if applicable)

### 2. Monitor

- [ ] Watch docs.rs build status
- [ ] Check GitHub Pages deployment
- [ ] Monitor crates.io download stats
- [ ] Watch for issues/questions

### 3. Maintenance

- [ ] Respond to issues on GitHub
- [ ] Update documentation based on feedback
- [ ] Plan next release

## Alpha Release Notes

BRRTRouter is currently in alpha stage (v0.1.0-alpha.1). This means:

### What Alpha Means

- **API Stability**: Breaking changes are expected between alpha releases
- **Documentation**: Published for review and feedback
- **Testing**: Seeking early adopters for testing and feedback
- **Production Use**: Not recommended until v0.1.0 stable

### Alpha Versioning

Alpha versions follow this pattern:
- `0.1.0-alpha.1` - First alpha release
- `0.1.0-alpha.2` - Second alpha release (with breaking changes)
- `0.1.0-alpha.N` - Nth alpha release
- `0.1.0-beta.1` - First beta release (API freeze)
- `0.1.0` - Stable release

### Feedback Channels

We're actively seeking feedback on:

1. **Documentation Quality**
   - Is the documentation clear and comprehensive?
   - Are examples helpful?
   - What's missing?

2. **API Design**
   - Are the APIs intuitive?
   - What would make them better?
   - Any ergonomic issues?

3. **Generated Code**
   - Is the generated code production-ready?
   - Are there patterns that should change?

4. **Performance**
   - Have you benchmarked it?
   - Any performance issues?

Please open issues at: https://github.com/microscaler/BRRTRouter/issues

## Version History

| Version | Date | Docs.rs | GitHub Pages | Notes |
|---------|------|---------|--------------|-------|
| 0.1.0-alpha.1 | TBD | 🔄 | ✅ | Initial alpha with Mermaid diagrams, seeking feedback |
| 0.1.0 | TBD | - | - | Planned stable release |

## Resources

- [Crates.io Publishing Guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [docs.rs Documentation](https://docs.rs/about)
- [GitHub Pages Deployment](https://docs.github.com/en/pages/getting-started-with-github-pages)
- [SemVer Specification](https://semver.org/)

## Questions?

If you have questions about publishing:

1. Check this guide first
2. Review the [Cargo Book](https://doc.rust-lang.org/cargo/)
3. Ask in GitHub Discussions
4. Contact maintainers

