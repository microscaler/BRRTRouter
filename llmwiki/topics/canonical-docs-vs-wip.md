# Canonical docs vs `docs/wip/` (staleness policy)

- **Status**: `verified`
- **Source docs**: [`llmwiki/docs-catalog.md`](../docs-catalog.md) (inventory), [`docs/DEVELOPMENT.md`](../../docs/DEVELOPMENT.md)
- **Code anchors**: n/a
- **Last updated**: 2026-04-17

## What it is

BRRTRouter’s `docs/` tree mixes **maintained** references (`ARCHITECTURE.md`, `CORS_OPERATIONS.md`, `DEVELOPMENT.md`, …) with a large **`docs/wip/`** historical and experiment tree (hundreds of files per [`docs-catalog.md`](../docs-catalog.md)).

## Policy for agents

1. **Default to non-wip** under `docs/` for anything that must match current behavior.
2. Treat **`docs/wip/**` as high staleness risk** — useful for archaeology, not as authority for “how it works today.”
3. Prefer **this wiki** + **reconciliation pages** (`llmwiki/reconciliation/`) + **code entry points** ([`reference/codebase-entry-points.md`](../reference/codebase-entry-points.md)) over random WIP markdown.
4. When a WIP doc contains a unique insight, **ingest** it into a wiki topic and cite the file, or add a reconciliation note — do not assume WIP is current.

## Cross-references

- [`topics/sibling-repos-and-wikis.md`](./sibling-repos-and-wikis.md)
