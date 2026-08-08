# Framework Maturity — Testing Standard

Same hard rules as [`../URI_REQUEST_TARGET/TESTING_STANDARD.md`](../URI_REQUEST_TARGET/TESTING_STANDARD.md):

1. Positive **and** negative unit tests required per story.
2. Name tests `*_positive_*` / `*_negative_*` (or story-prefixed).
3. No panics on hostile input.
4. Story “Unit tests” tables are the PR checklist.

| Area | Minimum |
|------|---------|
| Positive | ≥ 5 scenarios (or full table if domain smaller) |
| Negative | ≥ 5 scenarios |
| Regression | Lock at least one known footgun when applicable |

Status / limit / validation stories must assert **HTTP status + stable JSON error shape** where the surface is HTTP.
