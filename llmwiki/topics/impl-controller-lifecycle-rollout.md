# Impl controller lifecycle — Tier 1 rollout and backlog

- **Status**: `active` (2026-07-08)
- **PRD**: [`docs/PRD_IMPL_CONTROLLER_LIFECYCLE.md`](../../docs/PRD_IMPL_CONTROLLER_LIFECYCLE.md)
- **Code anchors**: `src/generator/impl_registry.rs`, `src/generator/migrate_registration.rs`, `src/server/app_config.rs`, `src/server/run_app.rs`, `templates/impl_registry.rs.txt`, `templates/impl_main.rs.txt`
- **Hauliage quoting/bidding controllers**: [`hauliage/microservices/bidding/impl/src/controllers/`](../../../hauliage/microservices/bidding/impl/src/controllers/) — **user-owned; never regen with `--force`**

## What shipped (Tier 1 Fix A)

| Layer | File | Role |
|-------|------|------|
| Gen mock | `{service}_gen/src/registry.rs` | `register_from_spec` — every route gets a stub |
| Impl override | `{service}_impl/src/impl_registry.rs` | `register_impl` — disk controllers replace matching routes |
| Main wiring | `{service}_impl/src/main.rs` | Calls both registries (3 lines after migration) |

CLI:

```bash
# First-time migration from manual match arms → impl_registry + main patch
brrtrouter-gen migrate-registration --spec … --output …/impl --apply

# After adding a new impl controller on disk (full discovery; never overwrites controller bodies)
brrtrouter-gen regen-impl-registry --spec … --output …/impl --apply
```

Default for `regen-impl-registry` is **dry-run**. Pass `--apply` to write. Omit `--regen-mod` unless you intentionally want `controllers/mod.rs` regenerated from disk.

## Bidding — current state (2026-07-08)

| Item | Status |
|------|--------|
| Tier 1 registration (`gen_registry` + `impl_registry`) | **Done** |
| All 6 quote controllers wired (incl. `save_draft_quote`) | **Done** |
| Controller bodies (`save_draft_quote.rs`, etc.) | **Untouched** — DB upsert, DRAFT status, nullable columns |
| Fix B `RunAppBuilder` main | **Done** — 638 → **67 lines**; lifeguard + DB warm hooks preserved |

### Why `save_draft_quote` was missing briefly

`migrate-registration` scopes wiring to handlers found in the **old** `main.rs` match loop (intentional for partial services like identity/org-mgmt). `save_draft_quote` existed on disk but was never in that loop, so the first migration wrote 5 arms. Fixed with `regen-impl-registry --apply` (registry only).

## Fix B — hauliage rollout complete

All **17** hauliage impl services use `RunAppBuilder` (~57–78 lines). Bidding uses lib-qualified `hauliage_bidding::controllers` (no `mod controllers` in bin); hooks match telemetry/customs:

- `extra_prometheus` → lifeguard metrics
- `before_listen` → `hauliage_database::db()` on main thread (may_postgres deadlock avoidance)

```bash
brrtrouter-gen migrate-main --output hauliage/microservices/<svc>/impl --apply
```

## Fix B — `run_app()` (BRRTRouter library)

| Module | Status |
|--------|--------|
| `brrtrouter::server::app_config` | Shared YAML types + `load_app_config()` |
| `brrtrouter::server::cors_setup` | CORS middleware at startup |
| `brrtrouter::server::security_setup` | Auth providers from config |
| `brrtrouter::server::run_app` | `RunAppBuilder::run()` — **shipped** |

**Pilot:** `hauliage/microservices/customs/impl/src/main.rs` — 66 lines, `cargo check` OK on ms02.

**Rollout (2026-07-08):** all **17** hauliage impl services on Fix B (~57–78 lines). Bidding migrated last (67 lines); quote controllers untouched.

## Cross-references

- [`reference/openapi-extensions.md`](../reference/openapi-extensions.md) — `x-brrtrouter-impl` tri-state
- [`topics/generator-cli-and-askama.md`](./generator-cli-and-askama.md) — CLI subcommands
- [`topics/sibling-repos-and-wikis.md`](./sibling-repos-and-wikis.md) — Hauliage wiki owns service seeds/BFF detail
- Hauliage scaffolding: [`hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md`](../../../hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md)
