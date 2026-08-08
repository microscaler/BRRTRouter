# Epic 16 — Release & observability maturity

**GitHub issue:** [#413](https://github.com/microscaler/BRRTRouter/issues/413)  
**Theme labels:** `release-maturity`, `epic`  
**Testing:** [`../TESTING_STANDARD.md`](../TESTING_STANDARD.md)  
**Board:** [`../BUILD_BOARD.md`](../BUILD_BOARD.md)

## Overview

Make BRRTRouter **externally consumable**: stable public API policy, crates.io
packaging polish, changelog/semver path out of alpha, and close observability
test gaps (fake OTEL coverage) called out on the ROADMAP.

**Does not include:** feature work from Epics 13–15; WebSocket.

## Success criteria (epic-level)

- [ ] Published API stability / semver policy.
- [ ] crates.io publish path documented and exercised (dry-run or publish).
- [ ] Fake OTEL (or equivalent) covers remaining critical test gaps.
- [ ] Beta/0.1 checklist complete for Sesame-style consumers.

## Wave plan

```text
Wave 0 ──► 16.1 API stability policy
Wave 1 ──► 16.2 crates.io packaging
Wave 2 ──► 16.3 observability test coverage
Wave 3 ──► 16.4 semver/changelog/beta checklist
Wave 4 ──► 16.5 consumer migration guide
```

## Stories

| Story | Title | Issue | Effort | Blocked by |
|-------|--------|-------|--------|------------|
| 16.1 | Public API stability policy | [#430](https://github.com/microscaler/BRRTRouter/issues/430) | S | — |
| 16.2 | crates.io packaging polish | [#431](https://github.com/microscaler/BRRTRouter/issues/431) | M | 16.1 |
| 16.3 | Observability test coverage (fake OTEL) | [#432](https://github.com/microscaler/BRRTRouter/issues/432) | M | — |
| 16.4 | Semver, changelog, beta checklist | [#433](https://github.com/microscaler/BRRTRouter/issues/433) | S–M | 16.1–16.2 |
| 16.5 | Consumer migration guide (alpha → 0.1) | [#434](https://github.com/microscaler/BRRTRouter/issues/434) | S | 16.4 |

## Functional requirements (epic)

| ID | Requirement |
|----|-------------|
| E-FR-1 | Document which modules are public-stable vs `#[doc(hidden)]` / unstable. |
| E-FR-2 | `cargo publish --dry-run` (or CI) succeeds for the liberary package set. |
| E-FR-3 | Critical telemetry paths covered by fake collector tests. |
| E-FR-4 | CHANGELOG + versioning policy for breaking changes. |
| E-FR-5 | Migration guide for Sesame-style apps. |

## Non-functional requirements (epic)

| ID | Requirement |
|----|-------------|
| E-NFR-1 | No secrets in published crates. |
| E-NFR-2 | CI gates for fmt/test/publish-dry-run as appropriate. |
| E-NFR-3 | Docs match published surface. |
