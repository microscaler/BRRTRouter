# Zero Trust — Build Board

**Theme:** Epic 14  
**Testing:** [`TESTING_STANDARD.md`](TESTING_STANDARD.md)

## Now / next

| Priority | ID | Status | Issue | Notes |
|----------|-----|--------|-------|-------|
| **DONE** | 14.1 | done | [#414](https://github.com/microscaler/BRRTRouter/issues/414) | Inventory & threat model |
| **NOW** | 14.2 | todo | [#415](https://github.com/microscaler/BRRTRouter/issues/415) | X.509 validate |
| **NEXT** | 14.3 | todo | [#416](https://github.com/microscaler/BRRTRouter/issues/416) | mTLS request path |

## Wave plan

```text
Wave 0 ──► 14.1
Wave 1 ──► 14.2 ‖ 14.3
Wave 2 ──► 14.4
Wave 3 ──► 14.5
Wave 4 ──► 14.6
Wave 5 ──► 14.7
Wave 6 ──► 14.8
```

## Story index

| ID | Title | Wave | Status | GitHub |
|----|--------|------|--------|--------|
| Epic 14 | SPIFFE X.509 / mTLS / Federation | — | todo | [#411](https://github.com/microscaler/BRRTRouter/issues/411) |
| 14.1 | Zero-trust inventory & threat model | 0 | done | [#414](https://github.com/microscaler/BRRTRouter/issues/414) |
| 14.2 | X.509 SVID parse & validate | 1 | todo | [#415](https://github.com/microscaler/BRRTRouter/issues/415) |
| 14.3 | mTLS peer identity on request path | 1 | todo | [#416](https://github.com/microscaler/BRRTRouter/issues/416) |
| 14.4 | X.509 SecurityProvider → authz | 2 | todo | [#417](https://github.com/microscaler/BRRTRouter/issues/417) |
| 14.5 | SVID/bundle rotation & fail-closed ready | 3 | todo | [#418](https://github.com/microscaler/BRRTRouter/issues/418) |
| 14.6 | SPIFFE Federation (bundles) | 4 | todo | [#419](https://github.com/microscaler/BRRTRouter/issues/419) |
| 14.7 | JWT SVID hardening (revocation + ECDSA) | 5 | todo | [#420](https://github.com/microscaler/BRRTRouter/issues/420) |
| 14.8 | Reference integration guide & e2e fixtures | 6 | todo | [#421](https://github.com/microscaler/BRRTRouter/issues/421) |
