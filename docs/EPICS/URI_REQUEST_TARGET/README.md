# URI / Request-Target Compliance Epics

**Source audit:** [docs/AUDIT-uri-request-target-and-rfc10008-2026-08.md](../../AUDIT-uri-request-target-and-rfc10008-2026-08.md)  
**Incident postmortem:** [docs/POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md](../../POSTMORTEM-proxy-query-encoding-invalid-uri-2026-08-07.md)  
**Summary:** [EPICS_AND_STORIES_SUMMARY.md](EPICS_AND_STORIES_SUMMARY.md)

Theme for making BRRTRouter’s **inbound request-target parsing** and **outbound URI
rebuild** fully compliant with RFC 3986 + RFC 9110 (+ WHATWG form-urlencoded
where we deliberately decode like browsers).

RFC 10008 (HTTP QUERY method) is **out of scope** for Epic 10 — see Epic 11.

## Epics

| Epic | Title | Directory | GitHub issue |
|------|--------|-----------|--------------|
| 10 | Request-target parse & rebuild — 100% URI compliance | [epic-10-uri-request-target-compliance/](epic-10-uri-request-target-compliance/) | _(create)_ |
| 11 | HTTP QUERY method (RFC 10008) | [epic-11-http-query-method/](epic-11-http-query-method/) | _(create)_ |

## Labels

- Theme: `uri-request-target`
- Epics: `epic`
- Stories: `story`

## Definition of “100% spec compliant” (Epic 10)

Done when every row in the Epic 10 compliance matrix is **Pass** under automated
tests (table + property/fuzz), and the audit doc scorecard is updated accordingly.
No known decode→encode→`Uri` failure for legal Unicode / reserved inputs; illegal
inputs fail closed with the correct status (not silent corruption, not 502 for
composition errors).
