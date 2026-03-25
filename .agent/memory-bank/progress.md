# Progress

## 2026-03-25 — Docker E2E: cross-process `flock` for pet_store build (fix flaky exit 139)

- **Cause:** `cargo nextest` runs multiple test **processes** in parallel; each called `ensure_image_ready()` and concurrently wrote `target/x86_64-unknown-linux-musl/release/pet_store` → corrupted binary → **SIGSEGV** in container (exit **139**). Failure looked tied to `ui_list_user_posts` but was startup/race, not that route.
- **Done:** `tests/curl_harness.rs` — `E2eDockerBuildLock` uses `libc::flock(LOCK_EX)` on `target/.pet_store_e2e_docker.lock` for the whole cargo + `docker build` sequence (Unix); no-op on non-Unix.

## 2026-03-25 — Tilt: `TILT_SKIP_OBSERVABILITY` for faster CI / kind

- **Done:** `Tiltfile` reads `TILT_SKIP_OBSERVABILITY` (`1` / `true` / `yes`) and omits observability YAML + `k8s_resource` entries; `build-brrtrouter` no longer waits on Prometheus/Loki/Promtail; `petstore` deps drop `prometheus` + `otel-collector`. Banner text adjusts. GitHub `tilt-ci` job sets `TILT_SKIP_OBSERVABILITY: "1"`.
- **Note:** Pet Store deployment still points OTLP at `otel-collector`; if skipped, export may log errors — OK for smoke tests.

## 2026-03-25 — `ui_secure_endpoint_bearer`: JWT payload base64url decode + E2E token

- **Cause:** `BearerJwtProvider` decoded the JWT payload with `base64::STANDARD`, which requires padding. Real JWTs (and jwt.io’s HS256 example) use **base64url without padding** → `InvalidPadding` → payload never parsed → validation always failed → **401** on `GET /secure`.
- **Done:** `decode_jwt_segment()` tries `URL_SAFE_NO_PAD`, then padded `STANDARD` (compat with `make_token` in tests). `PET_STORE_BEARER_DEV_TOKEN` in `tests/common/pet_store_e2e.rs` (third segment `sig` for mock `BearerJwtProvider`). `tests/spec_tests.rs`: `test_pet_store_secure_security_is_bearer_or_oauth2_not_and` guards OpenAPI OR semantics. Lib tests in `bearer_jwt.rs` for jwt.io payload + `.sig` token.
- **Verify:** `cargo test -p brrtrouter bearer_jwt::tests::`; `cargo test --test security_tests test_bearer_jwt_token_validation`; `cargo test --test ui_scenarios_pet_store ui_secure_endpoint_bearer` (Docker).

## 2026-03-24 — OpenAPI + config + tests: CORS documentation and `x-cors: inherit`

- **Done:** `examples/openapi.yaml` — `info.description` + `info.x-brrtrouter-cors` (origins from `config.yaml`); explicit `x-cors: inherit` on `list_pets`, `options_user`, `submit_form`, `get_matrix`, `register_webhook`. `templates/config.yaml` + generated `examples/pet_store/config/config.yaml` comments aligned. `PET_STORE_CORS_DEV_ORIGIN` in `tests/common/pet_store_e2e.rs`; `ui_scenarios_pet_store` imports it for preflight test. Ran `brrtrouter-gen generate --force`.

## 2026-03-24 — UI E2E: OPTIONS preflight `{}` not `null`; matrix URL `/matrix/1,2,3`

- **OPTIONS 500:** CORS short-circuit returned `HandlerResponse` with JSON `null`; `options_user` documents `200` + `type: object` → response validation failed (`null is not of type "object"`). **Done:** use `serde_json::json!({})` for all middleware OPTIONS `200` bodies (preflight success, CORS disabled OPTIONS, no `Origin` OPTIONS).
- **Matrix:** Radix matches `/matrix/{coords}` as two segments; `/matrix;coords=1,2,3` is one segment and does not match. **Done:** `ui_matrix_style_path` uses `{}/matrix/1,2,3`.

## 2026-03-24 — `response_body_schema_for_status`: fallback to `route.response_schema` when `responses` has no status entry

- **Cause:** `response_body_schema_for_status` used `route.responses.get(&status)?`, so an empty `responses` map (e.g. `CustomServerTestFixture` / legacy `response_schema` only) returned `None` and skipped JSON response validation — `test_response_body_validation_failure` saw 200 instead of 500.
- **Done:** Use `if let Some(status_map) = route.responses.get(&status) { ... }` then fall back to `route.response_schema` for 2xx when no per-status schemas match.
- **Verify:** `cargo test -p brrtrouter --test server_tests`.

## 2026-03-24 — JWKS: no re-fetch every `validate()` when `{"keys":[]}` (fix `test_jwks_empty_cache_no_retry_on_successful_empty_response`)

- **Cause:** `refresh_jwks_if_needed` used `needs_refresh = ttl_expired || cache_keys_empty`, so after a **successful** empty JWKS response the map stayed empty and every validation triggered another HTTP GET (2+ requests vs test expectation of 1).
- **Done:** `needs_refresh` is only `guard.0.elapsed() >= current_cache_ttl`. Failed refreshes leave the old timestamp, so TTL still drives retries.
- **Verify:** `cargo test -p brrtrouter --test security_tests test_jwks_empty_cache_no_retry_on_successful_empty_response` and `cargo test -p brrtrouter --test security_tests jwks_`.

## 2026-03-24 — CORS: POST `/webhooks` + `HandlerResponse::error` HTTP round-trip tests

- **Done:** `cors_middleware_tests::post_json_webhooks_invalid_origin_403_json_error_not_null` (POST, `register_webhook`, `application/json`, bad `Origin`). `response::tests::test_cors_handler_response_error_round_trip_not_null` uses `HandlerResponse::error` + `write_handler_response` like `AppService` (guards `403` + non-`null` body vs UI `POST /webhooks`).
- **Verify:** `cargo test -p brrtrouter --lib post_json_webhooks` and `cargo test -p brrtrouter --lib test_cors_handler_response_error_round_trip`.

## 2026-03-24 — Matrix path array: `1;2;3` decodes to `[1,2,3]` + tests

- **Cause:** `decode_param_value` used comma-only splitting for matrix-style array path params; segment `1;2;3` stayed one string → typed handler error `expected i32`.
- **Done:** For `ParameterStyle::Matrix` + array schema: strip optional `name=` prefix, split on `;` when present else on `,`. Unit test in `src/server/request.rs` (`test_decode_param_matrix_array_semicolons`); integration tests in `tests/param_style_tests.rs`.
- **Verify:** `cargo test -p brrtrouter --lib test_decode_param_matrix` and `cargo test --test param_style_tests`.

## 2026-03-24 — Unit tests: POST/form + CORS 403 (no `null`, reason `Forbidden`)

- **Done:** Lib tests for CORS `before()` invalid `Origin` on POST with `application/x-www-form-urlencoded` and on OPTIONS preflight — assert `HandlerResponse` body is `{"error":...}` not `Value::Null`. `HandlerResponse::error` test. `write_handler_response` 403 uses status line `403 Forbidden` and JSON object (regression vs `403 OK` + body `null`). **Verify:** `cargo test -p brrtrouter --lib invalid_origin` (and `error_response_is_json_object_not_null`, `test_write_handler_response_403_uses_forbidden_not_ok`, `post_form_urlencoded_allowed_origin`).

## 2026-03-24 — CI: `ui_scenarios_pet_store` mirrors sample-ui / curl scenarios

- **Done:** Integration crate `tests/ui_scenarios_pet_store.rs` exercises the same flows as the sample UI (pets, users, posts, admin, items, SSE, download, form, multipart, matrix, search, secure, webhooks, CORS invalid origin → 403 JSON). Shared helpers live in `tests/common/pet_store_e2e.rs` (used by `curl_integration_tests.rs`). `reqwest` test dependency includes `multipart`. Module-level `#![allow(dead_code)]` on `pet_store_e2e` avoids unused-helper warnings across split test binaries.
- **Verify:** `cargo test --test curl_integration_tests --test ui_scenarios_pet_store --no-run`. Full run needs Docker + same harness as curl tests (`tests/curl_harness.rs`).

## 2026-03-24 — CORS 403: JSON `null` body + wrong reason phrase

- **Cause:** Invalid `Origin` short-circuits with `HandlerResponse::new(403, …, Value::Null)` → body serializes to literal `null` (4 bytes). `status_reason` had no `403` → `"403 OK"` in HTTP line.
- **Done:** Use `HandlerResponse::error(403, "Origin not allowed by CORS policy")`. `status_reason`: `403` → `"Forbidden"`, `204` → `"No Content"`. Tests updated for `403`.

## 2026-03-24 — Response schema selection + form/multipart request bodies

- **Done:** `response_body_schema_for_status` skips `type: string` + `format: binary` when choosing a schema for JSON response validation (fixes multi-content `200` responses without relying only on cache keys). **`parse_request`** reads raw bytes, parses `application/x-www-form-urlencoded` into a JSON object, and treats `multipart/form-data` as `Some({})` so `request_body_required` no longer returns 400 when the body is not JSON. Tests in `server::request` for JSON/form/multipart.

## 2026-03-24 — Validator cache: schema digest (fix GET `/download/{id}` 500)

- **Root cause:** Response validators were keyed only by `handler + kind + status`, so OpenAPI operations with **multiple `content` types for the same status** (e.g. `image/png`, `application/octet-stream`, `application/json` for 200) **collided** — the first compiled schema (often `type: string` / binary) was reused for JSON responses → `"is not of type \"string\""` for a JSON object body.
- **Done:** Cache keys now append a **16-hex SHA-256 digest** of the JSON Schema bytes. Unit test `test_response_cache_same_status_different_schemas`, integration test `test_cache_same_status_different_response_schemas`. Docs updated in `validator_cache.rs`.

## 2026-03-24 — Typed `()` / JSON null + response validation (POST `/items`, `/webhooks`)

- **Done:** `#[handler]` without explicit `-> Response` now **compile_errors** (implicit `()` serializes to JSON `null`). Typed dispatch maps **null** JSON bodies to **500** with a clear message before response validation. **`response_body_schema_for_status`** only falls back to the default success schema for **2xx**; non-2xx bodies are not validated against the 200 schema unless documented per status. Removed duplicate **null** guard in `service.rs` (typed layer owns the fix). **Verify:** `cargo check -p brrtrouter -p brrtrouter_macros -p pet_store`; `cargo test -p brrtrouter --test spec_tests test_pet_store_post_item_response_schemas_are_objects`.

## 2026-03-24 — Generator `Cargo.toml` workspace deps (P2)

- **Done:** `write_cargo_toml_with_options` no longer skips config dependencies when `use_workspace_deps` is true and the crate is missing from `[workspace.dependencies]`. Per-crate explicit `version` / `path` / `git` specs are emitted with `use_workspace_deps == false` for that line. Pure `Workspace` specs (`workspace = true` only) error with a clear message instead of emitting invalid TOML.
- **Tests:** `test_write_cargo_toml_keeps_explicit_dep_not_in_workspace_dependencies`, `test_write_cargo_toml_errors_workspace_only_dep_missing_from_table` in `src/generator/tests.rs`.

## 2026-03-24 — `--sync` impl stubs + main gen: `$ref` response fields

- **Done:** Added `resolved_response_schema_json(spec, route)` in `src/generator/project/generate.rs` (uses `resolve_schema_ref` like the non-sync impl path). **`--sync`** now calls `extract_fields` on the resolved schema so `$ref` responses get proper `Response {{ ... }}` fields. The full impl-stub path and **`generate_project_with_options`** handler/controller `response_fields` use the same helper (avoids empty fields / E0063 from raw `$ref` blobs).

## 2026-03-24 — POST `/items/{id}` API Explorer + OpenAPI response schema

- **Done:** `sample-ui` split `/items` vs `/payment` default POST bodies — `/items` now uses `{ "name": "Test Item" }` (CreateItemRequest). POST `/items/{id}` **200/201** response schemas inlined (same as `Item`) so JSON Schema validation does not depend on `$ref` resolution for those responses. Regression test `test_pet_store_post_item_response_schemas_are_objects` in `tests/spec_tests.rs`.

## 2026-03-24 — POST `/webhooks` response validation (500 / null vs object)

- **Done:** Response validation now uses `response_body_schema_for_status(route, hr.status)` (prefer `responses[status].application/json`, else default). OpenAPI documents **`200`** for `register_webhook` (typed handlers default to 200) alongside **`201`**. Clear **500** when handler body is JSON `null`. Files: `src/server/service.rs`, `examples/openapi.yaml`, `examples/pet_store/doc/openapi.yaml`.

## 2026-03-24 — Validation logs: request/response `schema_path`

- **Done:** Replaced misleading hardcoded `#/components/schemas/request|response` in `src/server/service.rs` with `(operation requestBody)` / `(operation response schema)` — validators use inline or resolved operation schemas, not those component paths.

## 2026-03-24 — Goose CLI: `hatch-rate` → `increase-rate`

- **Done:** Current Goose (git pin) uses `--increase-rate` / `-r`, not `--hatch-rate`. Updated `Tiltfile`, `.github/workflows/ci.yml`, `justfile` (`goose-jsf`), `scripts/run_goose_tests.py`, `scripts/generate_benchmark_report.py`, and example module docs. Python CLIs accept `-r`, `--increase-rate`, and `--hatch-rate` (alias) with `dest='hatch_rate'`.
