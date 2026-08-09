## 2026-08-09 — Epic 13 complete (13.10 shipping)

**Shipped (Epic 13)**
- 13.1–13.9 previously done
- **13.10** Public TestApp (#410): `brrtrouter::test_support` behind Cargo feature `testing`
  - `TestApp::{from_service, from_spec, from_spec_with_options}`
  - `RequestBuilder` / `TestResponse` / `TestAppError`
  - Unit tests P1–P4, N1–N3, N5; pet_store integration smoke
  - Docs: `docs/TESTING.md`; board + story acceptance marked done
  - `just test` / `just nt` pass `--features testing`

**Gates:** `just test` + `just nt` green (2026-08-09).

**NOW:** Commit/push 13.10 + close #410. Next backlog: Epic 14 / 15 / 16 (not auto-started).
