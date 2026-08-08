# Zero Trust — Testing Standard

Same hard rules as [`../FRAMEWORK_MATURITY/TESTING_STANDARD.md`](../FRAMEWORK_MATURITY/TESTING_STANDARD.md):

1. Positive **and** negative unit tests required per story.
2. Name tests `*_positive_*` / `*_negative_*` (or story-prefixed).
3. No panics on hostile input.
4. Story “Unit tests” tables are the PR checklist.

Security / identity stories must assert **fail-closed** behavior and stable error/problem shapes where HTTP is involved.
