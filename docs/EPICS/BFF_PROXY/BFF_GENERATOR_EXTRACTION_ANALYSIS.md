# BFF Generator: Extract to BRRTRouter Tooling?

**Purpose:** Decide whether to extract the BFF generator from RERP (or equivalent) into BRRTRouter’s Python tooling so **any consumer** has a standard set of BFF tools. RERP can then either **import** the Python module from BRRTRouter or **call** BRRTRouter tooling directly (CLI).

**Related:** Epic 1 Stories 1.2 (BFF generator proxy extensions), 1.3 (BFF generator components/security merge), **1.4 (Extract BFF generator to BRRTRouter tooling)** — implementation story for extraction, test migration, and RERP update; `docs/BFF_PROXY_ANALYSIS.md` §3.4, §5.6.

---

## 1. Goal

- **Standard BFF tooling:** One canonical BFF spec generator that emits OpenAPI 3.1 BFF specs consumable by BRRTRouter (with `x-brrtrouter-downstream-path`, `x-service`, merged `components.parameters` / `securitySchemes` / root `security`).
- **Any consumer** can use it without depending on RERP.
- **RERP** can either:
  - **Embed:** Import the Python module from BRRTRouter tooling (e.g. `from brrtrouter_tooling.bff import generate_bff_spec`) and call it from RERP’s own flow, or
  - **Call:** Invoke BRRTRouter CLI (e.g. `brrtrouter bff generate --suite-config bff-suite-config.yaml --output openapi/bff.yaml`) and use the generated spec.

---

## 2. Current State

| Location | Description |
|----------|-------------|
| **RERP** | BFF spec generation lives in RERP (e.g. `generate_system.py`, `bff-suite-config.yaml`). Merges paths/schemas, adds `x-service` and `x-service-base-path`; does **not** yet emit `x-brrtrouter-downstream-path` or merge components/security (Epic 1.2, 1.3). |
| **BRRTRouter tooling** | `tooling/` provides `brrtrouter` CLI (today: `dependabot automerge`). No BFF generator. |
| **BRRTRouter validator** | `brrtrouter-validator-python/` validates OpenAPI specs; referenced for integration with “bff-generator” (separate repo or RERP). |

So today the BFF generator is **RERP-specific**; BRRTRouter only consumes the generated spec. Extracting it would make the generator **BRRTRouter-owned** and reusable.

---

## 3. Options

### Option A — Extract BFF generator into BRRTRouter tooling (recommended)

- **Where:** New package under `tooling/src/brrtrouter_tooling/bff/` (e.g. `generate.py`, `merge.py`, `config.py`) and a CLI subcommand `brrtrouter bff generate`.
- **What it does:**
  - Reads a suite config (e.g. YAML: list of services with `spec_path`, `base_path`, optional `port`).
  - Discovers and merges sub-service OpenAPI specs (paths, schemas) with prefixing.
  - For each operation: sets `x-brrtrouter-downstream-path` (exact path on downstream) and `x-service` (service key) — **Story 1.2**.
  - Merges or injects `components.parameters`, `components.securitySchemes`, and root `security` — **Story 1.3**.
  - Writes a single BFF OpenAPI spec (and optionally runs BRRTRouter validator).
- **Consumers:**
  - **Any project:** `pip install brrtrouter-tooling` then `brrtrouter bff generate ...` or `from brrtrouter_tooling.bff import generate_bff_spec`.
  - **RERP:** Either (1) add `brrtrouter-tooling` as a dependency and call `generate_bff_spec(suite_config, output_path)` or (2) shell out to `brrtrouter bff generate ...` and use the output.

**Pros:** Single source of truth; BRRTRouter and BFF contract stay aligned; any consumer gets the same behaviour; RERP can still use it by import or CLI.  
**Cons:** BRRTRouter repo owns and maintains the generator; RERP may need to adapt its current flow (paths, config shape) to match the tooling’s interface.

### Option B — Keep BFF generator in RERP only

- RERP extends its existing `generate_system.py` (or equivalent) to add `x-brrtrouter-downstream-path`, `x-service`, and components/security merge (Stories 1.2, 1.3).
- BRRTRouter only consumes the spec; no BFF generator in BRRTRouter.

**Pros:** No extraction; RERP keeps full control.  
**Cons:** Other consumers must reimplement or fork; BFF contract and BRRTRouter can drift; no standard “BRRTRouter BFF tooling” for the ecosystem.

### Option C — Hybrid: BRRTRouter provides library, RERP keeps orchestration

- BRRTRouter tooling provides a **library** (e.g. `brrtrouter_tooling.bff.merge_specs`, `add_proxy_extensions`, `merge_components_security`) that implements the merge and extension logic.
- RERP (or any consumer) keeps its own **orchestration** (discovery, config, file layout) and calls the library. Optionally, BRRTRouter also adds a thin CLI that uses the same library (e.g. `brrtrouter bff generate` with a standard config format).

**Pros:** Reusable logic in BRRTRouter; consumers can still customize discovery and config.  
**Cons:** Two layers to maintain; config/suite format may still diverge unless the CLI is the “standard” and RERP aligns to it.

---

## 4. Recommendation

**Prefer Option A (extract into BRRTRouter tooling)** so that:

1. **Any consumer** has a standard BFF generator and config format (suite config → BFF spec).
2. **BRRTRouter** owns the contract: same tooling that emits `x-brrtrouter-downstream-path` and `x-service` is the one BRRTRouter expects (Epic 1.1, 2.2).
3. **RERP** can:
   - **Import:** Add `brrtrouter-tooling` as a dependency and call the generator as a library (e.g. in a RERP script that then runs BRRTRouter codegen), or
   - **Call:** Run `brrtrouter bff generate ...` and use the generated spec in RERP’s pipeline (e.g. as input to BRRTRouter codegen or Tilt).
4. **Stories 1.2 and 1.3** are implemented **inside BRRTRouter tooling**; RERP either uses that implementation or delegates to it.

If extraction is too heavy in the short term, **Option C** is a compromise: implement the merge and extension logic in `brrtrouter_tooling.bff` as a library first, add a minimal CLI that uses it, and let RERP call the library (or CLI) until RERP’s own generator is deprecated or aligned.

---

## 5. If We Extract: Where and How

### 5.1 Place in BRRTRouter repo

| Item | Location |
|------|----------|
| **Python package** | `tooling/src/brrtrouter_tooling/bff/` — e.g. `__init__.py`, `config.py` (suite config schema), `merge.py` (merge paths/schemas), `extensions.py` (add `x-brrtrouter-downstream-path`, `x-service`), `components.py` (merge parameters, securitySchemes, security). |
| **CLI** | `brrtrouter bff generate --suite-config <path> --output <path>` (and optional flags: `--validate`, `--base-path-prefix`). Wire in `tooling/src/brrtrouter_tooling/cli/main.py` (e.g. `command == "bff"` → `bff.generate()`). |
| **Config format** | Document a standard suite config (YAML) that lists services with `name`, `spec_path`, `base_path`, optional `port`; align with RERP’s `bff-suite-config.yaml` where possible so RERP can pass the same file or a thin adapter. |
| **Tests** | `tooling/tests/test_bff.py` — unit tests for merge, extensions, components merge; optional integration test with a fixture suite config. |

### 5.2 How RERP consumes it

| Mode | Usage |
|------|--------|
| **Import** | `pip install brrtrouter-tooling` (or install from BRRTRouter repo). In RERP: `from brrtrouter_tooling.bff import generate_bff_spec`; call with RERP’s suite config (or adapt config to the standard shape). RERP’s `generate_system.py` (or equivalent) becomes a thin wrapper that calls BRRTRouter and then runs RERP-specific steps (e.g. copy files, trigger BRRTRouter codegen). |
| **CLI** | RERP (or CI) runs `brrtrouter bff generate --suite-config openapi/accounting/bff-suite-config.yaml --output openapi/accounting/bff/openapi.yaml`. Downstream steps use the generated spec. No Python import from RERP; only the `brrtrouter` binary (and standard config format). |

Both modes give RERP a single, standard BFF generator; RERP chooses whether to embed via import or call the CLI.

---

## 6. Summary

| Question | Answer |
|----------|--------|
| **Do we need to extract the BFF generator into BRRTRouter tooling?** | **Recommended: yes.** Gives any consumer a standard BFF tool set and keeps the BFF contract aligned with BRRTRouter. |
| **Where does it live?** | `tooling/src/brrtrouter_tooling/bff/` + CLI `brrtrouter bff generate`. |
| **How does RERP use it?** | Either **import** the Python module from `brrtrouter-tooling` or **call** `brrtrouter bff generate ...` directly. |
| **What about Epic 1.2 and 1.3?** | Implement them in BRRTRouter tooling; RERP then uses that implementation (library or CLI) instead of maintaining its own generator logic. |

---

## 7. References

- Epic 1: [Spec-driven proxy](epic-1-spec-driven-proxy/README.md) — Stories 1.2, 1.3, **1.4 (extract to BRRTRouter tooling, migrate tests, update RERP)**
- [BFF_PROXY_ANALYSIS.md](../BFF_PROXY_ANALYSIS.md) §3.4, §5.2, §5.6
- OPENAPI_3.1.0_COMPLIANCE_GAP.md §8 (components/security merge)
- BRRTRouter tooling: `tooling/README.md`, `tooling/src/brrtrouter_tooling/cli/main.py`
