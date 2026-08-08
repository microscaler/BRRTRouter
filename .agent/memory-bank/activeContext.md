## 2026-08-09 — Epic 13.2 rate limiting shipped

**Commit:** `b0b1ba2` · Issue [#402](https://github.com/microscaler/BRRTRouter/issues/402) closed.

- `RateLimitMiddleware` token bucket (DashMap); default **off**
- Enable: `rate_limit:` in `config.yaml` (`enabled`, `requests`, `window_secs`, `routes`, `key`)
- Metric: `brrtrouter_rate_limit_sheds_total`
- JWT boundary unchanged: consumer/enforcer only

**NOW:** 13.3 RFC 7807 (#403) · 14.2 X.509 SVID (#415)  
**JWT:** Sesame/external IdP issues; see `docs/JWT_AND_IDENTITY_BOUNDARY.md`
