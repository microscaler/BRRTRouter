# PRD: Impl Controller Lifecycle — Safe Stub Generation and Registration

**Project:** BRRTRouter  
**Document version:** 1.0  
**Date:** 2026-07-08  
**Status:** Draft — for review  
**Related:**

- Hauliage [`PRD_BFF_SCAFFOLDING_REMEDIATION.md`](../../hauliage/docs/PRD_BFF_SCAFFOLDING_REMEDIATION.md) (F5 eradication, Tier 1 D4–D7)
- Hauliage [`PRD_BFF_VIEW_COMPOSITION.md`](../../hauliage/docs/PRD_BFF_VIEW_COMPOSITION.md) §7.6–§7.8 (Tier 1–3, manifest, `imara-diff`)
- Hauliage [`docs/llmwiki/topics/scaffolding-lifecycle.md`](../../hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md) (F5, F8)
- [`BUILDING_WITH_BRRTRouter.md`](../BUILDING_WITH_BRRTRouter.md), [`llmwiki/flows/code-generation-flow.md`](../llmwiki/flows/code-generation-flow.md)
- [`src/generator/project/generate.rs`](../src/generator/project/generate.rs) (`generate_impl_stubs`, `USER_OWNED_SENTINELS`)

---

## Implementation snapshot (2026-07-08)

| Item | State |
|------|--------|
| `generate-stubs` creates missing impl controller files | Shipped |
| Skip existing stubs unless `--force` | Shipped |
| `--force` preserves files with comment sentinel | Shipped (`// BRRTRouter: user-owned`, `// BRRTROUTER_USER_OWNED`, `// Implemented`) |
| `--sync` patches `Response { … }` block only (non-SSE, non-array) | Shipped (brittle string/brace matching) |
| Skip proxy-only stubs when `x-service` + downstream path set | Shipped |
| Tier 1: generated `impl/src/impl_registry.rs` from disk discovery | **Shipped** (2026-07-08) |
| Tier 1: `brrtrouter::server::run_app()` collapsed `main.rs` | **Shipped** — all hauliage impls incl. bidding (67 lines) |
| Tier 2: manifest + 3-way merge (`imara-diff`) | **Not started** |
| `brrtrouter-gen plan-impl` / `status` CLI | **Partial** (`plan-impl` shipped) |
| `x-brrtrouter-impl` tri-state validation | **Shipped** (errors on `true` without file) |

**Incidents motivating this PRD:**

- **F8 (2026-07-08):** Hauliage GICS four controllers overwritten by `generate-stubs --force` → empty stubs; restored from git.
- **F5 (2026-04-17):** 35 hauliage routes served gen mocks despite real impl files on disk (missing `main.rs` match arms).

---

## Table of contents

1. [Executive summary](#1-executive-summary)
2. [Problem statement](#2-problem-statement)
3. [Goals and non-goals](#3-goals-and-non-goals)
4. [Current architecture](#4-current-architecture)
5. [Failure modes catalog](#5-failure-modes-catalog)
6. [Target architecture (tiers)](#6-target-architecture-tiers)
7. [Functional requirements](#7-functional-requirements)
8. [Non-functional requirements](#8-non-functional-requirements)
9. [Acceptance criteria](#9-acceptance-criteria)
10. [Implementation phases](#10-implementation-phases)
11. [Risks and mitigations](#11-risks-and-mitigations)
12. [Open questions](#12-open-questions)
13. [References](#13-references)

---

## 1. Executive summary

BRRTRouter’s **gen/impl split** is intentional: OpenAPI drives generated types and mock stubs in `gen/`; product teams implement business logic in `impl/src/controllers/`. Today, **three separate mechanisms** try to keep gen and impl aligned — and all are fragile:

1. **Manual `main.rs` match arms** (Register & Overwrite) → F5 silent mocks  
2. **`generate-stubs --force`** → F8 clobber of real implementations  
3. **Comment sentinels + `--sync`** → Tier 0 stopgap; no manifest, brittle `Response {` patching  

This PRD defines **Tier 1** (generated registration, no hand-maintained route index) and **Tier 2** (manifest + 3-way merge) so consumers (Hauliage, Sesame-IDAM, RERP) can regen safely after OpenAPI changes without losing business logic or silently serving mocks.

---

## 2. Problem statement

| Stakeholder need | Current gap |
|------------------|-------------|
| Add OpenAPI operation → get typed `Request`/`Response` in `gen/` | Works via `brrtrouter-gen generate` |
| Create starting impl stub for new handler | Works via `generate-stubs` (no flags) |
| **Implement** handler without losing code on regen | Requires manual sentinel comment; easy to forget |
| OpenAPI **schema change** on existing handler → update impl signature | `--sync` only patches mock `Response { … }` literal; breaks on real implementations |
| New impl file → route actually dispatches to it | Requires manual `main.rs` arm → F5 if forgotten |
| BFF orchestration route must not be generated as downstream **proxy** stub | OpenAPI `x-service` tags cause proxy template; `--force` overwrites orchestration |
| CI/Tilt stub regen must be safe by default | Was `--force` in Tilt until 2026-07-08 fix |

**Root cause:** Generator ownership boundaries are **file-granular in theory** but **not enforced in tooling**. Users must remember sentinels, match arms, and OpenAPI proxy tags simultaneously.

---

## 3. Goals and non-goals

### 3.1 Goals

1. **G1:** Regenerating stubs MUST NOT destroy user business logic without an explicit, reviewable conflict.
2. **G2:** New or existing impl controller files MUST be registered for dispatch **without** hand-edited `main.rs` match lists (Tier 1).
3. **G3:** CLI MUST report planned changes before writing (`plan`, `status`).
4. **G4:** OpenAPI schema drift MUST surface as **compile errors** (via `gen` types + `#[handler]`) or **actionable merge conflicts**, not silent runtime mocks.
5. **G5:** Proxy-only BFF routes and BFF-orchestration routes MUST be distinguishable in OpenAPI so codegen picks the correct template.
6. **G6:** Backward-compatible migration for existing hauliage/sesame/rerp services without mass manual rewrites.

### 3.2 Non-goals (initial release)

- AST-level 3-way merge (`syn`) — deferred; line merge first (see hauliage PRD §7.8)
- Region markers inside shared files (Tier 3) — park unless Tier 1+2 insufficient
- Automatic migration of business logic from monolithic controllers into split wire/body files
- Changing `gen/` overwrite semantics (always full regen from spec)
- Replacing Askama template engine

---

## 4. Current architecture

### 4.1 Crate layout (consumers)

```text
microservices/<service>/
├── gen/                    # GENERATOR-OWNED — full regen from OpenAPI
│   └── src/handlers/       # Request, Response types
│   └── src/controllers/    # Mock stubs (example JSON)
├── impl/
│   ├── src/controllers/    # USER-OWNED business logic
│   └── src/main.rs         # MIXED — wiring + hand match arms (problem)
└── openapi/…/openapi.yaml
```

### 4.2 Stub generation today

Source: [`generate_impl_stubs`](../src/generator/project/generate.rs)

| Flag | Behaviour |
|------|-----------|
| (none) | Create stub if missing; **skip** existing |
| `--force` | Overwrite unless sentinel present |
| `--sync` | Patch `Response { … }` on sentinel files only |

Sentinels: `USER_OWNED_SENTINELS` in `generate.rs`.

Proxy skip: if route has `x-service` **and** `x-brrtrouter-downstream-path`, stub generation skipped (gen crate still gets proxy controller).

### 4.3 Registration today (ADR 0001)

```rust
registry::register_from_spec(&mut dispatcher, &routes);  // all gen stubs
for route in &routes {
    match route.handler_name.as_ref() {
        "my_handler" => { /* spawn impl controller */ }
        _ => {}
    }
}
```

---

## 5. Failure modes catalog

| ID | Name | Symptom | Cause |
|----|------|---------|-------|
| **F1** | Orphan OpenAPI op | Route 404 or gen-only mock forever | No impl file, no intent to implement |
| **F2** | Stale `main.rs` index | New op added; old match list | Generator doesn't own registration |
| **F3** | Schema drift | `cargo check` error on `req.data.field` | **Desired signal** — gen types changed |
| **F4** | Orphan impl file | File exists, not in `mod.rs` | Manual file create |
| **F5** | Silent mock | Real impl on disk; gen stub serves traffic | Impl file exists, **no match arm** |
| **F6** | Wrong security/CORS wiring | Partial `main.rs` regen | Mixed-ownership `main.rs` |
| **F7** | Template upgrade conflict | Generator adds import/span; user edited same lines | No merge layer |
| **F8** | Stub clobber | DB logic → `Response(vec![])` | `--force` without sentinel |
| **F9** | Proxy overwrite | BFF orchestration → `proxy_untyped` only | OpenAPI proxy tags on orchestrated POST |

---

## 6. Target architecture (tiers)

### Tier 0 — Sentinels (shipped, retain)

- Comment sentinels + safe default CLI (no `--force` in Tilt/CI)
- Document in consumer `AGENTS.md`
- **Does not satisfy G1/G2 alone**

### Tier 1 — Generated registration (Fix A + Fix B)

**Fix A:** New template `templates/impl_registry.rs.txt` → output `impl/src/impl_registry.rs`

- Scans `impl/src/controllers/*.rs` on disk (or manifest of handler names from OpenAPI ∩ existing files)
- Emits `register_impl(dispatcher, routes)` with one arm per discovered controller
- Uses static match from sorted handler list — **no manual index**

**Two registry files (BRRTRouter only — not `cylon-skills` registry):**

| Crate | Template | Output | Function |
|-------|----------|--------|----------|
| `{service}_gen` | `registry.rs.txt` | `gen/src/registry.rs` | `register_from_spec` — mock/example controllers |
| `{service}_impl` | `impl_registry.rs.txt` | `impl/src/impl_registry.rs` | `register_impl` — overrides with disk-discovered impl |

**Fix B:** `brrtrouter::server::run_app(AppConfig)` library function

- Owns CORS, security, metrics, dispatcher setup
- `impl/src/main.rs` ≤ ~40 lines: parse args, load config, call `run_app`

**OpenAPI:** `x-brrtrouter-impl` tri-state:

| Value | Meaning |
|-------|---------|
| `true` | Real impl required; error at regen if controller file missing |
| `false` | Gen stub only; never expect impl file |
| omitted | Warning if impl file exists (legacy) |

**Blocks:** F2, F5, F6 (partially)

### Tier 2 — Manifest + 3-way merge

**Artifacts:**

- `impl/.brrtrouter/manifest.json` — per-handler `user_modified`, `schema_version`, template hash
- `impl/.brrtrouter/snapshots/controllers/<handler>.rs.snapshot` — last THEIRS (gitignored, optional)

**Merge:** BASE (snapshot) + OURS (disk) + THEIRS (Askama fresh stub) via **`imara-diff`**

| Case | Action |
|------|--------|
| OURS == BASE | Write THEIRS (never edited) |
| OURS != BASE, THEIRS == BASE | Keep OURS |
| Both changed | 3-way merge or conflict markers + exit 1 |

**CLI:**

- `brrtrouter-gen plan [--service …]` — dry-run report
- `brrtrouter-gen regen [--service …]` — apply Tier 2 merge
- `brrtrouter-gen status` — template/schema drift vs manifest

**Optional:** `syn` + `quote` pass for `--sync-v2` — update fn signature + imports only, preserve body (narrower than full merge).

**Blocks:** F7, F8 (without relying on comments), F9 (with OpenAPI lint)

### Tier 3 — Region markers (parked)

`// <BRRTR:BEGIN gen_registration>` — only if Tier 1 cannot split file boundaries.

---

## 7. Functional requirements

### 7.1 Stub generation (`generate-stubs`)

| ID | Requirement |
|----|-------------|
| **FR-STUB-01** | Default invocation MUST NOT overwrite existing controller files. |
| **FR-STUB-02** | `--force` MUST NOT overwrite files marked user-modified in manifest (Tier 2) or with sentinel (Tier 0/1 until manifest ships). |
| **FR-STUB-03** | `--sync` MUST update only generator-owned regions: imports, handler signature, `Response` literal in **stub** handlers; MUST NOT replace non-stub function bodies. |
| **FR-STUB-04** | Generator MUST NOT emit impl stub for routes marked BFF-orchestration (new extension `x-brrtrouter-orchestration: true` or absence of downstream path when `x-brrtrouter-impl: true` on BFF). |
| **FR-STUB-05** | `plan` MUST list: create, skip, preserve, merge, conflict per handler. |
| **FR-STUB-06** | New stub template MUST document sentinel requirement in header comments. |

### 7.2 Registration (Tier 1)

| ID | Requirement |
|----|-------------|
| **FR-REG-01** | Generated `impl/src/impl_registry.rs` MUST register every controller module that exists on disk and has a matching OpenAPI `operationId`. |
| **FR-REG-02** | Registration MUST occur after `register_from_spec` (overwrite pattern preserved). |
| **FR-REG-03** | Controller discovery MUST use `operationId` = module filename (`snake_case`). |
| **FR-REG-04** | Untyped handlers (`pub fn handle(req: HandlerRequest)`) and typed (`#[handler]`) MUST both be supported in registry emission. |
| **FR-REG-05** | If `x-brrtrouter-impl: true` and controller file missing → `plan`/`generate` exits non-zero with clear error. |

### 7.3 Manifest (Tier 2)

| ID | Requirement |
|----|-------------|
| **FR-MAN-01** | Manifest MUST record OpenAPI spec hash and BRRTRouter template version per service. |
| **FR-MAN-02** | Per-handler entries MUST track `user_modified`, `schema_version`, `last_stub_hash`. |
| **FR-MAN-03** | Successful regen MUST update manifest and snapshot atomically (write temp + rename). |
| **FR-MAN-04** | Conflicts MUST leave conflict markers in file and exit non-zero unless `--accept-ours` / `--accept-theirs` strategy specified. |

### 7.4 OpenAPI extensions

| ID | Requirement |
|----|-------------|
| **FR-OAPI-01** | Document `x-brrtrouter-impl`, `x-brrtrouter-orchestration`, proxy tags in [`llmwiki/reference/openapi-extensions.md`](../llmwiki/reference/openapi-extensions.md). |
| **FR-OAPI-02** | Linter (existing or new) MUST warn when `x-brrtrouter-impl: true` combined with `x-service` downstream on same operation (BFF orchestration anti-pattern). |

### 7.5 Consumer migration

| ID | Requirement |
|----|-------------|
| **FR-MIG-01** | Tier 1 rollout MUST NOT require rewriting controller bodies — only `main.rs` replacement + generated `impl_registry.rs`. |
| **FR-MIG-02** | Provide `brrtrouter-gen migrate-registration` to emit registry from existing controllers + validate match arm parity. | **Shipped** (`--apply` patches simple main.rs) |
| **FR-MIG-04** | Provide `brrtrouter-gen regen-impl-registry` for full disk discovery after migration (never overwrites controller bodies). | **Shipped** (bidding `save_draft_quote` wired 2026-07-08) |
| **FR-MIG-05** | Collapse hauliage `impl/main.rs` via Fix B `run_app()`. | **Shipped** — all 17 hauliage impls including bidding (67 lines, 2026-07-08) |
| **FR-MIG-03** | Hauliage Tilt `stubs-*` resources MUST NOT pass `--force` by default (verified 2026-07-08). |

---

## 8. Non-functional requirements

| ID | Category | Requirement |
|----|----------|-------------|
| **NFR-01** | Safety | Zero unplanned overwrites of user-modified controllers in acceptance test suite. |
| **NFR-02** | Safety | `plan` output MUST be deterministic for same inputs (spec hash + disk + template version). |
| **NFR-03** | Performance | `plan` for 100 handlers completes in < 2s on dev laptop. |
| **NFR-04** | DX | Conflict markers use familiar git conflict format (`<<<<<<<`, `=======`, `>>>>>>>`). |
| **NFR-05** | Compatibility | Tier 1 ships without Tier 2; Tier 2 builds on Tier 1 manifest paths. |
| **NFR-06** | Testing | Unit tests for merge cases (OURS=BASE, conflict, clean merge); integration test on `pet_store` example. |
| **NFR-07** | Docs | Update `BUILDING_WITH_BRRTRouter.md`, consumer `AGENTS.md`, hauliage `scaffolding-lifecycle` wiki. |
| **NFR-08** | Dependencies | Tier 2 adds `imara-diff` only; Tier 1 adds **no** new runtime deps to `brrtrouter` library. |
| **NFR-09** | CI | Consumer CI MAY run `brrtrouter-gen plan` and fail on unexpected `would_overwrite` without sentinel/manifest. |

---

## 9. Acceptance criteria

### 9.1 Tier 1 complete when

- [ ] **AC-T1-01:** `templates/impl_registry.rs.txt` renders registry with N arms for N controller files in `pet_store` example.
- [ ] **AC-T1-02:** Hauliage `fleet` `impl/src/main.rs` ≤ 50 lines after regen; zero hand-maintained handler match arms.
- [ ] **AC-T1-03:** F5 parity test: add new controller file → regen → route dispatches to impl without editing `main.rs`.
- [ ] **AC-T1-04:** `x-brrtrouter-impl: true` without file → `generate` fails with handler name in stderr.
- [ ] **AC-T1-05:** Existing hauliage services pass `cargo check` after registry migration (fleet sentinel first, then roll all).

### 9.2 Tier 2 complete when

- [ ] **AC-T2-01:** `brrtrouter-gen plan` reports `preserve` for GICS-style user-modified controllers without sentinel (manifest `user_modified: true`).
- [ ] **AC-T2-02:** Simulated OpenAPI field add on stub-only handler → THEIRS applied automatically (OURS == BASE).
- [ ] **AC-T2-03:** Simulated template + user edit on same lines → conflict markers; exit code 1.
- [ ] **AC-T2-04:** Re-run of `regen` with no spec change is no-op (manifest hash unchanged).
- [ ] **AC-T2-05:** Documented rollback: `--accept-ours` restores pre-regen disk state.

### 9.3 Tier 0 regression (must hold through Tier 1/2)

- [ ] **AC-T0-01:** `generate-stubs --force` on sentinel file → preserved (existing test or new).
- [ ] **AC-T0-02:** Tilt/CI default stub command does not pass `--force` (hauliage Tiltfile verified).

### 9.4 End-to-end consumer scenario

- [ ] **AC-E2E-01:** Developer adds OpenAPI field to `list_gics_sectors` response → `plan` shows `sync` or merge on gen types only; controller body with DB query unchanged after `regen`.
- [ ] **AC-E2E-02:** Developer runs `hauliage gen stubs gics` (no flags) → no changes to four protected GICS controllers.
- [ ] **AC-E2E-03:** New handler `get_foo` added to spec → stub created; Tier 1 registry picks it up without `main.rs` edit.

---

## 10. Implementation phases

### Phase 0 — Documentation + Tier 0 hardening (2026-07-08, partial)

- [x] Sentinel list includes `BRRTROUTER_USER_OWNED`
- [x] Consumer `AGENTS.md` (hauliage, sesame, rerp, BRRTRouter)
- [x] Hauliage Tilt `stubs-*` without `--force`
- [x] Wiki checkpoints (account-first onboarding paused)
- [x] Unit test: `--force` + sentinel → preserved
- [x] `plan-impl` stub that lists skip/preserve counts (read-only precursor)

### Phase 1 — Tier 1 registration (~3–5 days)

1. [x] Implement `impl_registry.rs.txt` + disk discovery
2. [x] Implement `run_app()` extraction (Fix B) — pilot on hauliage **customs** (66-line main)
3. [x] Tri-state `x-brrtrouter-impl` validation in generator
4. [x] Migrate `pet_store` (tests), hauliage `fleet` sentinel (registry + main wiring)
5. [ ] Parity test vs F5 audit methodology

### Phase 2 — Tier 2 manifest + merge (~5–8 days)

1. Manifest schema + snapshot storage
2. Integrate `imara-diff` 3-way merge
3. Ship `plan`, `regen`, `status` subcommands
4. Pilot on `gics` service (recent F8 victim)
5. Optional `syn`-based signature sync prototype

### Phase 3 — Consumer rollout

1. Regenerate all hauliage services (registry migration)
2. Sesame-IDAM org-mgmt + identity-login consumer controllers
3. RERP documentation + CI `plan` gate
4. Deprecate comment-only sentinel in docs (manifest preferred)

---

## 11. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Tier 1 auto-registers intentionally disabled handlers | `x-brrtrouter-impl: false` + no file; or explicit `x-brrtrouter-disabled: true` |
| Line merge corrupts Rust syntax | Run `rustc --parse` or `cargo check` in regen CI step; never commit on failure |
| Manifest drift if gitignored snapshots lost | Snapshots derivable from regen; manifest committed |
| Multi-consumer BRRTRouter version skew | Manifest records `brrtrouter_template_version`; `status` command |
| Breaking `main.rs` customizations in older services | `migrate-registration` diff report before apply |

---

## 12. Open questions

1. **Registry discovery:** Match by filename only, or require `#[handler(FooController)]` / `pub fn handle` export trait?
2. **Untyped vs typed registry:** Single code path or separate templates?
3. **Commit snapshots?** Default gitignore vs commit for reproducible BASE without re-render?
4. **RERP scale:** 71 services × manifest — monorepo-wide `plan` performance?
5. **`linkme` vs generated match:** Use linkme for registration instead of generated match table? (compile-time vs simplicity tradeoff)

---

## 13. References

### Code

- [`src/generator/project/generate.rs`](../src/generator/project/generate.rs) — `generate_impl_stubs`, sentinels
- [`src/generator/templates.rs`](../src/generator/templates.rs) — `sync_impl_stub_response`, Askama writers
- [`templates/impl_controller_stub.rs.txt`](../templates/impl_controller_stub.rs.txt)
- [`brrtrouter_macros/src/lib.rs`](../brrtrouter_macros/src/lib.rs) — `#[handler]` macro

### Hauliage

- [`docs/PRD_BFF_SCAFFOLDING_REMEDIATION.md`](../../hauliage/docs/PRD_BFF_SCAFFOLDING_REMEDIATION.md)
- [`docs/PRD_BFF_VIEW_COMPOSITION.md`](../../hauliage/docs/PRD_BFF_VIEW_COMPOSITION.md) §7.6–§7.8
- [`docs/F5_AUDIT_2026-04-17.md`](../../hauliage/docs/F5_AUDIT_2026-04-17.md)
- [`docs/llmwiki/topics/account-first-onboarding-checkpoint.md`](../../hauliage/docs/llmwiki/topics/account-first-onboarding-checkpoint.md) — paused consumer work

### Sesame-IDAM

- [`docs/llmwiki/topics/topic-account-first-onboarding-checkpoint.md`](../../seasame-idam/docs/llmwiki/topics/topic-account-first-onboarding-checkpoint.md)

### External

- [imara-diff](https://github.com/pascalkuthe/imara-diff) — proposed Tier 2 merge engine
- [Karpathy llm-wiki pattern](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) — wiki checkpoint convention

---

**Review checklist for approver:**

- [ ] Tier 1 scope acceptable as first milestone (before Tier 2 merge)?
- [ ] `x-brrtrouter-orchestration` extension naming OK?
- [ ] Acceptance criteria sufficient for hauliage + sesame pilot?
- [ ] Open questions — decisions needed before Phase 1 start?
