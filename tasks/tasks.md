# BRRTRouter Development Tasks & Farm Tools Guide

## 🚀 Farm Tools - CLI-First Development Protocol

### Core Principle: ZERO Shell Scripts Policy
- **NEVER write Python scripts to bypass CLI commands** - This defeats the purpose of having a robust CLI system
- **ALWAYS use `farm` commands for all operations** - Every operation must go through the proper CLI interface
- **CLI commands are the ONLY acceptable interface** - Direct module imports are forbidden for operational tasks
- **Test code paths by using them** - CLI commands ensure all code paths are exercised and validated
- **Fix CLI issues, don't bypass them** - If CLI commands have import errors or bugs, FIX THE CLI, don't work around it

### Essential Farm Commands Reference

#### Session Management (MANDATORY STARTUP)
```bash
# ALWAYS run first - loads Memory Bank context
farm agent startup

# Check Memory Bank health
farm agent memory-bank status

# Save current context
farm agent memory-bank save
```

#### Development Workflow
```bash
# Intelligent AI routing to best model
farm ai "your question"

# Run Python tests with rich output
farm test python test_file.py

# Pre-commit quality checks
farm git preflight

# Full preflight checks
farm preflight
```

#### Code Quality & Testing
```bash
# Fix Python linting issues
farm lint python --fix

# Generate coverage reports
farm coverage python

# Check shell→Python migration progress
farm migration status

# Run tests with coverage analysis
farm test python --coverage
```

#### Git Operations (NEVER use git directly)
```bash
# Create new branch
farm git create-branch feature/branch-name

# Stage and commit changes
farm git commit -m "commit message"

# Create pull request
farm git create-pr --title "PR Title" --body "Description"

# Update PR by branch
farm git update-pr-by-branch --branch branch-name --body-file description.md

# Check workflow status
farm git workflow-status

# Pre-commit formatting fixes
farm git preflight --fix
```

#### Environment & Configuration
```bash
# Decrypt SOPS environment variables
farm env decrypt

# Setup Ollama models
farm setup models

# Check Docker infrastructure
farm docker health

# Full environment restoration
farm env setup complete-setup
```

#### AI Development
```bash
# Let AI router choose best model
farm ai "debug this error"

# View AI usage costs
farm ai-cost report

# Direct model interaction
farm ollama chat qwen2.5-coder
```

### Emergency Commands
```bash
# Restore corrupted Memory Bank
farm agent memory-bank init

# Restart all services
farm docker restart

# System health check
farm status health
```

## 🎯 BRRTRouter Current Task Status

### Phase 1: Code Quality Foundation ✅ COMPLETE
- [x] Fixed all 72+ clippy warnings
- [x] Resolved module inception issues
- [x] Enhanced May coroutines compliance
- [x] Added missing feature flags
- [x] Improved safety documentation
- [x] Fixed tracing middleware for May's threading model
- [x] Created PR #157 with comprehensive documentation

**Results:** 27 tests (24 passed, 3 ignored), zero clippy warnings, clean compilation

### Phase 2: Test Coverage Enhancement 🔄 IN PROGRESS
- [ ] Boost test coverage to 90%+ (currently partial)
- [ ] Add comprehensive unit tests for all modules
- [ ] Enhance integration test coverage
- [ ] Add performance regression tests
- [ ] Document test patterns and conventions

### Phase 3: Template System Improvements 📋 PENDING
- [ ] Improve Askama template system for code generation
- [ ] Enhance template error handling
- [ ] Add template validation
- [ ] Optimize template performance

### Phase 4: Documentation & Generated Code 📚 PENDING
- [ ] Add comprehensive documentation warnings about generated code
- [ ] Create developer onboarding guide
- [ ] Document May coroutines architecture decisions
- [ ] Add API documentation generation

## 🏗️ Architecture Guidelines

### May Coroutines Compliance
- **Respect May's threading model** - All async code must be May-compatible
- **No std::thread usage** - Use May's coroutine primitives
- **Proper error propagation** - Ensure errors bubble up through May's stack
- **Tracing integration** - Use May-aware tracing middleware

### Code Organization Rules
- **Rust code ONLY in /components directory** - No exceptions
- **TypeScript ONLY in /ui portal** - No experimental TS outside UI
- **Zero shell scripts** - Everything must use Python farm tools
- **TDD principles** - Write tests before code, minimum 65% coverage target

### Testing Requirements
- **Minimum 65% test coverage** - Core rule, target 80%
- **Test-driven development** - Tests before implementation
- **May coroutines testing** - All async tests must use May runtime
- **Integration tests** - Cover end-to-end scenarios

## 📋 Development Workflow

### Starting a New Session
1. **Load Memory Bank:** `farm agent startup`
2. **Activate Python environment:** `source .venv/bin/activate`
3. **Check project status:** `farm status health`
4. **Review current tasks:** Review this file and PRD

### Making Changes
1. **Create feature branch:** `farm git create-branch feature/description`
2. **Write tests first:** Follow TDD principles
3. **Implement changes:** Maintain May coroutines compliance
4. **Run quality checks:** `farm git preflight --fix`
5. **Commit changes:** `farm git commit -m "description"`
6. **Create PR:** `farm git create-pr`

### Quality Gates
- ✅ Zero clippy warnings
- ✅ All tests passing
- ✅ Minimum 65% test coverage
- ✅ May coroutines compliance
- ✅ Farm tools usage only
- ✅ No shell scripts created

## 🎯 Performance Goals
- **Target:** 1M requests/second on Raspberry Pi 5
- **Architecture:** May coroutines for maximum efficiency
- **Benchmarking:** Use `benches/throughput.rs` for validation

## 📖 Key Documents
- **PRD:** `tasks/code-quality-foundation-prd.md` - Comprehensive project requirements
- **Roadmap:** `docs/ROADMAP.md` - Long-term project vision
- **Architecture:** `docs/ADRS/` - Architecture decision records

## 🔧 Troubleshooting

### Common Issues
1. **Import errors:** Use `farm env decrypt` to refresh environment
2. **Test failures:** Check May coroutines compatibility
3. **Clippy warnings:** Run `farm lint python --fix`
4. **Memory Bank issues:** Use `farm agent memory-bank init`

### Getting Help
- Use `farm ai "describe your problem"` for intelligent assistance
- Check farm command help: `farm <command> --help`
- Review Memory Bank: `farm agent memory-bank status`

---

*Remember: This project follows CLI-first development with zero shell scripts. Always use farm tools for all operations. Maintain May coroutines architecture throughout all implementations.* 