# Active Context

## 2026-04-13 — CORS per-route disabled metric

- **Added:** `brrtrouter_cors_route_disabled_total` on `/metrics` (`MetricsMiddleware::inc_cors_route_disabled` / `cors_route_disabled()`), incremented once per request when `RouteCorsPolicy::Disabled` (`x-cors: false`) in `CorsMiddleware::before`.
- **Tests:** `test_cors_metrics_sink_route_disabled`; metrics endpoint asserts new series; docs: `CORS_OPERATIONS.md`, `CORS_IMPLEMENTATION_AUDIT.md`.

## 2026-03-25 — `ui_secure_endpoint_bearer` / `BearerJwtProvider` JWT payload decoding

- **Fixed:** Payload segments are decoded with **base64url (no padding)** first, then padded standard base64 for internal test tokens. E2E uses `PET_STORE_BEARER_DEV_TOKEN` (third segment `sig` matching default mock signature).
- **Memory bank:** See `progress.md` entry for 2026-03-25.

## 2025-03-24 — Flake `test_jwks_sub_second_cache_ttl_timing_accuracy`

- **Cause:** Test used `cache_ttl(200ms)` then slept **250ms** before the validation burst. By then `guard.0.elapsed() >= ttl`, so `refresh_jwks_if_needed` treated the cache as expired and issued extra JWKS HTTP fetches (2 vs 3+ depending on scheduling).
- **Fix:** Use **600ms** TTL and **120ms** warmup so the burst stays **well under** TTL; sleep **TTL + 200ms** before the post-expiry validate. Assert `<= 3` for the hot phase (initial + retry tolerance). Extended mock server accept loop to 32.
