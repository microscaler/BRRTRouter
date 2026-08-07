# URI / Request-Target — Testing Standard (mandatory for Epics 10–11)

Every story **must** ship comprehensive **unit tests** before it can be marked
Done. Integration tests are welcome; they do **not** replace unit coverage.

## Hard rules

1. **Positive and negative** cases are both required. A story with only happy-path
   tests is incomplete.
2. Tests live next to the code under test (`#[cfg(test)]` in the module, or a
   focused `tests/*.rs` file). Name them so CI failure points at the story
   (e.g. `parse_query_params_positive_*`, `parse_query_params_negative_*`).
3. Each Acceptance Criterion that says “behaviour X” must map to at least one
   **asserted** unit test (table-driven preferred).
4. **No panics** on hostile input: negative suites must include truncated `%`,
   illegal bytes, empty strings, and oversized targets where applicable.
5. Golden / property tests from Stories 10.1 and 10.10 are additive; story-local
   unit tests remain required.
6. Story README “Unit tests” section is the checklist reviewers use in PR.

## Positive vs negative (definitions)

| Class | Meaning |
|-------|---------|
| **Positive** | Legal input per the cited spec; expect successful parse/encode/proxy composition and semantically correct output. |
| **Negative** | Illegal, ambiguous, or hostile input; expect fail-closed behaviour (error/status), **no** silent corruption, **no** panic. Also: inputs that “parse” but would corrupt semantics if left unencoded (e.g. raw `&` in a value) — assert the **safe** encoding path. |

## Minimum bar per story

| Area | Minimum |
|------|---------|
| Positive cases | ≥ 5 distinct scenarios (or full golden table if smaller domain) |
| Negative cases | ≥ 5 distinct scenarios |
| Regression | At least one test named/locked to a known incident or audit row when applicable |

Stories may exceed these minima; they must not fall short.

## Story doc checklist (before PR / Done)

Every story markdown file under this theme **must** include:

1. Link to this standard (`**Testing standard:** …`).
2. A **## Unit tests (required)** section with:
   - **### Positive** table (ID | Scenario | Assert)
   - **### Negative** table (ID | Scenario | Assert)
   - **### Acceptance criteria (tests)** checkboxes
3. Parent Acceptance criteria checkbox: *Unit tests section complete (positive + negative)*.

Reviewers reject Done without the tables implemented as named `cargo test` cases
(or docs-fixture tests where Story 11.4 is documentation-only).
