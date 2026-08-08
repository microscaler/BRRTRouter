# Archived one-shot scripts

Historical migration helpers that used to live in the **repo root**. They are kept
for archaeology only — do **not** run them as part of normal development.

| File | Was for |
|------|---------|
| `fix_tests.py` | Bulk-insert `queue_guard: None` into `HandlerRequest { … }` literals |
| `fix_dispatcher.py` | Same, scoped to `tests/dispatcher_tests.rs` |
| `patch_names.py` | Add `.name(...)` to `may` coroutine builders |

Prefer `git` history if you need to understand when those fields landed.
