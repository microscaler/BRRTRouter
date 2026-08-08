## 2026-08-09 — Epic 13.6 handler deadlines shipped

**Issue:** [#406](https://github.com/microscaler/BRRTRouter/issues/406)

- `src/dispatcher/deadline.rs` — resolve + 504 problem; env `BRRTR_HANDLER_DEADLINE_MS`
- Dispatcher `recv_timeout`; route `x-brrtrouter-deadline-ms`; metric callback
- Docs: `docs/HANDLER_DEADLINES.md`
- Tests: unit + `handler_deadline_integration_tests`

**NOW:** 13.7 SSE live flush (#407) · also 14.2 X.509 SVID (#415)
