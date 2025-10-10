# Upstream PR Tracker

Track the status of our upstream contributions to fix the TooManyHeaders issue.

## Quick Status

| Repository | Fork | Branch | PR | Status | Notes |
|------------|------|--------|----|---------| ------|
| httparse | [microscaler/httparse](https://github.com/microscaler/httparse) | `docs/header-buffer-sizing` | TBD | 📝 Not Started | Documentation PR |
| may_minihttp | [microscaler/may_minihttp](https://github.com/microscaler/may_minihttp) | `feat/configurable-max-headers` | TBD | 📝 Not Started | Code change PR |

## Status Legend

- 📝 **Not Started**: Haven't created fork/PR yet
- 🔄 **In Progress**: Fork created, working on changes
- ✅ **PR Created**: Pull request submitted to upstream
- 💬 **Under Review**: Maintainer reviewing/discussing
- 🎯 **Approved**: PR approved, awaiting merge
- ✨ **Merged**: Changes merged to upstream!
- ❌ **Closed**: PR closed/rejected
- 🔀 **Using Fork**: BRRTRouter using our fork

## Detailed Status

### httparse - Documentation PR

**Goal**: Add guidance on header buffer sizing for production use

**Changes**:
- Add "Header Buffer Sizing" section to README
- Provide recommended buffer sizes
- Update example to use 128 headers
- Document memory impact
- Include industry standards

**Timeline**:
- [ ] Fork repository
- [ ] Create branch
- [ ] Make changes
- [ ] Test changes
- [ ] Submit PR
- [ ] Address review feedback
- [ ] Get merged

**Links**:
- Upstream: https://github.com/seanmonstar/httparse
- Fork: https://github.com/microscaler/httparse (not created yet)
- PR: TBD

**Notes**:
- Low risk PR (documentation only)
- High chance of acceptance
- Maintainer (seanmonstar) is active
- No breaking changes

---

### may_minihttp - Configurable MAX_HEADERS

**Goal**: Make MAX_HEADERS configurable at compile-time, default to 128

**Changes**:
- Make `MAX_HEADERS` configurable via `MAX_HTTP_HEADERS` env var
- Change default from 16 to 128
- Add comprehensive documentation
- Add usage examples
- Document memory/performance impact

**Timeline**:
- [ ] Fork repository
- [ ] Create branch
- [ ] Implement changes
- [ ] Write tests
- [ ] Update documentation
- [ ] Submit PR
- [ ] Address review feedback
- [ ] Get merged

**Links**:
- Upstream: https://github.com/Xudong-Huang/may_minihttp
- Fork: https://github.com/microscaler/may_minihttp (not created yet)
- PR: TBD

**Notes**:
- Medium risk PR (code change)
- Breaking change: None (backwards compatible)
- Memory impact: +4.4KB per request
- Performance impact: None
- Need to sell the benefit clearly

---

## BRRTRouter Integration Status

### Current State

**Vendored Patch**:
- ✅ Currently using vendored `may_minihttp` with `MAX_HEADERS = 128`
- ⚠️ Vendor directory causes build issues (removed vendoring config)
- 🔄 Need to switch to fork via git dependency

**Next Steps**:
1. [ ] Remove vendor directory
2. [ ] Update Cargo.toml to use fork
3. [ ] Test build
4. [ ] Verify no TooManyHeaders errors

### After Upstream Merge

Once `may_minihttp` PR is merged:

```toml
# Cargo.toml - revert to upstream
[dependencies]
may_minihttp = "0.1.12"  # or whatever version includes fix
```

## Communication Plan

### Before Submitting PRs

- [ ] Review both codebases thoroughly
- [ ] Check existing issues/PRs for similar requests
- [ ] Prepare benchmarks/evidence
- [ ] Write clear, professional PR descriptions

### During Review

- [ ] Respond promptly to feedback
- [ ] Be flexible on implementation details
- [ ] Provide additional data if requested
- [ ] Keep PRs focused and minimal

### If PRs are Rejected

**Plan A**: Maintain our forks
- Document fork maintenance process
- Set up automated sync with upstream
- Regularly rebase on upstream changes
- Consider becoming maintainer

**Plan B**: Switch HTTP libraries
- Evaluate `hyper` (100 header default)
- Evaluate `actix-web` (32 header default)
- Major refactor but clean solution

## Success Metrics

### Minimum Success ✅
- [x] Identified root cause (httparse via may_minihttp)
- [x] Implemented workaround (vendored patch)
- [x] Documented solution
- [ ] Created forks with fixes
- [ ] BRRTRouter using forks (no TooManyHeaders errors)

### Target Success 🎯
- [ ] httparse docs PR merged
- [ ] may_minihttp PR merged
- [ ] BRRTRouter using upstream versions
- [ ] No TooManyHeaders errors in production

### Stretch Success 🚀
- [ ] Become trusted contributor to may_minihttp
- [ ] Help improve httparse ergonomics
- [ ] Present findings at Rust meetup
- [ ] Blog post about debugging process

## Lessons Learned

### What Worked Well
- Systematic investigation (vendor/ inspection)
- Clear documentation of findings
- Temporary workaround while pursuing upstream fix
- Preparing detailed PR plan before submitting

### What Could Be Better
- Should have checked upstream earlier
- Could have reached out to maintainers first
- Vendoring all dependencies caused issues

### For Future Reference
- Always check if issue is in dependencies first
- Document investigation process thoroughly
- Have a rollback plan before making changes
- Community contributions benefit everyone

## Resources

- Investigation docs: `docs/TOO_MANY_HEADERS_INVESTIGATION.md`
- Current patch: `vendor/may_minihttp/src/request.rs`
- PR plan: `docs/UPSTREAM_PR_PLAN.md`
- Commands: `docs/FORK_AND_PR_COMMANDS.md`
- Testing: `docs/TEST_HEADER_LIMITS.md`

## Updates Log

| Date | Event | Notes |
|------|-------|-------|
| 2025-10-10 | Created tracker | Initial planning phase |
| TBD | Forked repositories | |
| TBD | Submitted PRs | |
| TBD | PRs under review | |
| TBD | PRs merged | 🎉 |

