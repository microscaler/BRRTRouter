# BRRTRouter Tasks Directory

This directory contains comprehensive Product Requirements Documents (PRDs) and task specifications for BRRTRouter development.

## Documents

### [Code Quality & Foundation PRD](./code-quality-foundation-prd.md)
Comprehensive PRD for stabilizing BRRTRouter's foundation before implementing new roadmap features.

**Key Focus Areas:**
- **68 clippy warnings** resolution
- **90%+ test coverage** implementation
- **May coroutines architecture** compliance
- **Template system** improvements
- **Generated code** documentation

## Quick Start

1. **Read the PRD**: Start with `code-quality-foundation-prd.md` for complete context
2. **Review Current State**: Understand the 68 clippy warnings and test coverage gaps
3. **Follow Phases**: Implement in order - Code Quality → Tests → Templates → Documentation
4. **Respect Architecture**: All implementations must use May coroutines, not tokio/async-std

## Critical Constraints

### May Coroutines Only
- All async operations must use `may` runtime
- No tokio or async-std dependencies allowed
- Stack size configuration must be respected
- Panic recovery required for all handlers

### Code Generation
- Files in `examples/` are auto-generated - **DO NOT EDIT**
- Templates in `templates/` control generated code
- All generated files need "DO NOT EDIT" warnings
- Template changes affect all generated examples

## Success Criteria

- [ ] Zero clippy warnings with `-D warnings`
- [ ] 100% test pass rate (no ignored tests)
- [ ] 90%+ test coverage across all modules
- [ ] Clear documentation for May coroutines usage
- [ ] Robust template system with validation

## Timeline

- **Week 1-2**: Fix clippy warnings and module structure
- **Week 2-3**: Achieve 90%+ test coverage
- **Week 3-4**: Improve template system and generated code
- **Week 4-5**: Complete documentation and polish

## Getting Started

```bash
# Check current state
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo fmt --check

# Start with Phase 1 tasks from the PRD
# Focus on module structure and safety documentation first
```

For detailed implementation guidance, see the comprehensive PRD document. 