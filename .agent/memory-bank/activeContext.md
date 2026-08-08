## 2026-08-08 — Epic 12 Wave 4 shipped (12.8)

- **12.8** Perf science Phase 6 — #399 closed.
- `src/perf_harness.rs` + benches `match_vs_validate`, `request_guards`; Criterion sample/measurement stabilized.
- Evidence (ms02 release): match ~161ns, `is_valid` ~44ns, `iter_errors` ~163ns — all sub-µs; **no trie rewrite**.
- Next bottleneck: full `AppService` / dispatch (Phase 3 reply-slot), not radix.
- Docs: `docs/PERFORMANCE.md` § Phase 6, `docs/flamegraph.md`; Photon `docs/perf.md`.
- Epic 12 board: all stories **done**.

## 2026-08-08 — Epic 12 Wave 3 shipped

- **12.6** multipart → JSON fields; #397. **12.7** HttpNoContent / HEAD omit; #398.

## 2026-08-08 — Photon is suite home; brochure moved

- Suite brochure: **`microscaler/photon`**. BRRTRouter `website/` pointer only.
