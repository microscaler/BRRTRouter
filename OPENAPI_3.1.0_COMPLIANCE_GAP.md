# BRRTRouter OpenAPI 3.1.0 Compliance Gap Analysis

This document catalogs what is **outstanding** in BRRTRouter to achieve full OpenAPI 3.1.0 support. It is intended to guide the "Deep dive into OpenAPI spec" work referenced in the README.

**References:**
- [OpenAPI 3.1.0 Spec](https://spec.openapis.org/oas/v3.1.0)
- BRRTRouter uses the **oas3** Rust crate (v0.20) for parsing; oas3 targets OpenAPI 3.1.x (3.0 may have limited compatibility).

---

## 1. Implemented (in BRRTRouter)

| Area | Status | Notes |
|------|--------|-------|
| **Root: info, servers, paths, security, tags** | ✅ | Used for base path, title→slug, security, tags |
| **Paths + path item** | ✅ | `get`, `post`, `put`, `delete`, `patch`, `options`, `head`, `trace`; path-level `parameters` |
| **Operation: operationId, parameters, requestBody, responses, security** | ✅ | `extract_request_schema`, `extract_response_schema_and_example`, `extract_parameters`, `extract_security_schemes` |
| **Operation: deprecated** | ⚠️ | Parsed by oas3; not surfaced in `RouteMeta` or used in codegen/runtime |
| **Components: schemas** | ✅ | `#/components/schemas/X` resolved in `resolve_schema_ref`, `expand_schema_refs` |
| **Components: parameters** | ✅ | `#/components/parameters/X` resolved in `resolve_parameter_ref`, `extract_parameters` |
| **Components: securitySchemes** | ✅ | `extract_security_schemes`; ApiKey, HTTP (basic/bearer), OAuth2, OpenID Connect, RemoteApiKey, Spiffe |
| **Parameter: path, query, header, cookie** | ✅ | `ParameterLocation`, style, explode |
| **Request/response: application/json schema** | ✅ | Request/response validation; `estimate_body_size` |
| **$ref in requestBody/response content** | ✅ | Resolved to `#/components/schemas/` |
| **Vendor: x-handler-*, x-sse, x-cors, x-brrtrouter-stack-size, x-brrtrouter-body-size-bytes** | ✅ | Used in build, CORS, generator |
| **Path item $ref** | ⚠️ | Depends on oas3; not explicitly resolved in BRRTRouter (paths are iterated as from parser) |
| **Response: 1xx, 2xx, 3xx, 4xx, 5xx, default** | ✅ | All status codes in `Responses`; `extract_response_schema_and_example` prioritizes 200 application/json |
| **MediaType: example, examples** | ✅ | `MediaTypeExamples::Example`, `Examples` in `extract_response_schema_and_example` |
| **JSON Schema: type, format, properties, items, required, allOf, oneOf, anyOf** | ✅ | Used in generator/schema and validation; `type: ["string","null"]` for optional in 3.1 style (see schema.rs) |

---

## 2. Outstanding: Document and Top-Level

| Feature | OAS 3.1 | BRRTRouter | Notes |
|---------|---------|------------|-------|
| **webhooks** | `webhooks: { [name]: PathItem \| $ref }` | ❌ Not read | 3.1 addition; incoming webhooks. Parser may support; no route/build logic. |
| **jsonSchemaDialect** | Root `jsonSchemaDialect` | ❌ Ignored | Default `$schema` for Schema Objects. Schema handling assumes OAS/JSON Schema 2020-12; no dialect switch. |
| **externalDocs** | Root, tag, operation, schema | ❌ Ignored | Doc only; no impact on routing/codegen. |
| **info.termsOfService, license, summary** | Info object | ⚠️ Partial | `title`, `version` used; `description`, `contact` in meta. termsOfService, license, summary not used. |

---

## 3. Outstanding: Components

| Feature | OAS 3.1 | BRRTRouter | Notes |
|---------|---------|------------|-------|
| **components.responses** | `#/components/responses/X` | ❌ Not resolved | Operation `responses` can $ref here. `extract_response_schema_and_example` only handles inline or (implied) schema $ref; no `resolve_response_ref` for `#/components/responses/`. |
| **components.requestBodies** | `#/components/requestBodies/X` | ❌ Not resolved | Linter checks `requestBody` $ref starts with `#/components/requestBodies/` but returns early; `extract_request_schema` does not resolve it. Request body would be missed. |
| **components.headers** | `#/components/headers/X` | ❌ Not used | Response headers; not used in build or codegen. |
| **components.examples** | `#/components/examples/X` | ⚠️ Partial | MediaType `examples` can $ref here. oas3 may resolve; BRRTRouter uses `examples` for default example only. |
| **components.links** | `#/components/links/X` | ❌ Ignored | Response `links`; design-time only; no runtime. |
| **components.callbacks** | `#/components/callbacks/X` | ❌ Ignored | Reusable callback defs; see Operation callbacks. |
| **components.pathItems** | `#/components/pathItems/X` | ❌ Not resolved | 3.1; paths can $ref `#/components/pathItems/X`. No resolution in BRRTRouter. |

---

## 4. Outstanding: Paths and Operations

| Feature | OAS 3.1 | BRRTRouter | Notes |
|---------|---------|------------|-------|
| **Path Item $ref** | `$ref` to Path Item | ⚠️ oas3? | If oas3 resolves, we may get path; else missing. |
| **Path item: servers** | Override servers for path | ❌ Ignored | Only root `servers[0].url` used for `base_path`. |
| **Operation: callbacks** | `callbacks: { [expr]: PathItem }` | ❌ Ignored | Out-of-band callbacks; runtime expression → URL. No parsing or runtime. |
| **Operation: servers** | Override servers for op | ❌ Ignored | Same as path-level. |
| **Operation: externalDocs** | Doc link | ❌ Ignored | Doc only. |

---

## 5. Outstanding: Parameters and Encoding

| Feature | OAS 3.1 | BRRTRouter | Notes |
|---------|---------|------------|-------|
| **Parameter: content** | `content: { media: MediaType }` | ⚠️ Unclear | Parameter can use `content` instead of `schema`. `extract_parameters` uses `param.schema`; `content`-based params may not be fully handled. |
| **Parameter: allowReserved, allowEmptyValue** | Parameter object | ⚠️ Unclear | Passed through oas3→ParameterMeta? Not obviously used in validation. |
| **Parameter: deprecated** | Parameter object | ❌ Not in ParameterMeta | — |
| **Encoding (media type)** | `encoding` in MediaType | ❌ Ignored | multipart / form-urlencoded encoding; no encoding-specific handling. |

---

## 6. Outstanding: Schema (JSON Schema 2020-12 / 3.1)

| Feature | OAS 3.1 | BRRTRouter | Notes |
|---------|---------|------------|-------|
| **nullable** | Deprecated; use `type: ["T","null"]` | ⚠️ | schema.rs has 3.1-style `type` array for optional; `nullable` may still be parsed by oas3. Prefer 3.1. |
| **$schema, jsonSchemaDialect** | Per-schema / root | ❌ | No dialect or $schema handling. |
| **contentEncoding, contentMediaType** | In schema (3.1 / JSON Schema) | ❌ | File/binary; format has no effect on content-encoding per 3.1. No use in validation or codegen. |
| **examples** (array at schema) | Replaces deprecated `example` | ⚠️ | `example` used in generator; `examples` array not clearly handled. |
| **Schema: discriminator** | oneOf/anyOf/allOf | ⚠️ | oas3 may have it; generator/validation use of discriminator for polymorphism not verified. |
| **Schema: xml** | XML Object | ❌ | Doc/serialization only; not used. |

---

## 7. Outstanding: Response and Links

| Feature | OAS 3.1 | BRRTRouter | Notes |
|---------|---------|------------|-------|
| **Response: headers** | `headers: { [name]: Header \| $ref }` | ❌ | Not in ResponseSpec; no codegen for response headers. |
| **Response: links** | `links: { [name]: Link \| $ref }` | ❌ | Design-time; no use. |
| **Response $ref** | `#/components/responses/X` | ❌ | Not resolved (see 3.). |
| **Link, runtime expressions** | `operationRef`, `operationId`, `parameters`, `requestBody` | ❌ | Not implemented. |

---

## 8. BFF Generator and Downstream Impact

When using **bff-generator** to produce a BFF spec consumed by BRRTRouter:

| BFF-generator gap | Effect on BRRTRouter |
|-------------------|----------------------|
| **components.parameters not merged** | BFF has `$ref: '#/components/parameters/Page'` in paths but no `components.parameters`. `resolve_parameter_ref` returns `None` → param is **dropped** (no panic). |
| **components.securitySchemes not merged** | BFF has no `securitySchemes` → `extract_security_schemes` is empty → no auth for BFF unless added elsewhere. |
| **security (root) not merged** | BFF does not set `security`; if embedded script did, BRRTRouter would use it. |
| **Shared Error schema / components** | bff-generator does not add a shared `Error` schema; BRRTRouter does not require it, but docs/contracts may. |

**Recommendation:** Extend bff-generator to:
1. Merge **components.parameters** from all service specs (and optionally a shared set, e.g. Page, Limit, Search).
2. Allow **metadata** (or config) to inject **components.securitySchemes** and root **security** so BFF specs remain usable with BRRTRouter security.

---

## 9. oas3 Parser Limitations

- oas3 targets 3.1; 3.0 may have parse gaps. When in doubt, validate with official 3.1 examples.
- **Path Item $ref**, **components.pathItems**, **components.requestBodies**, **components.responses**: whether oas3 pre-resolves or leaves `Ref` must be verified; BRRTRouter does not currently resolve them.

---

## 10. Suggested Implementation Order

1. **High impact, BFF/real specs**
   - **components.parameters** in BFF: implement merge in bff-generator (and/or resolve `#/components/parameters/` when missing in BFF).
   - **components.requestBodies**: add `resolve_request_body_ref` and use in `extract_request_schema`.
   - **components.responses**: add `resolve_response_ref` and use in `extract_response_schema_and_example`.

2. **Medium impact**
   - **components.pathItems** and Path Item `$ref`: resolve so paths from `$ref` are included.
   - **Path/operation servers**: optional override of `base_path` or server URL when building routes.
   - **Parameter `content`**: handle `content`-based parameters in `extract_parameters`.

3. **Lower priority / doc-only**
   - **webhooks**: only if BRRTRouter should route incoming webhook requests.
   - **callbacks**: out-of-band; would need runtime expression evaluation and side-car behavior.
   - **externalDocs, links, tags.externalDocs**: documentation tooling.

4. **JSON Schema 3.1**
   - **contentEncoding / contentMediaType**: if file/binary upload or non-JSON responses need 3.1 semantics.
   - **$schema / jsonSchemaDialect**: if supporting non–2020-12 schemas.

---

## 11. Relationship to bff-generator

- **bff-generator** produces an OpenAPI 3.1.0 BFF spec. For BRRTRouter to consume it without subtle breakage:
  - BFF must include **components.parameters** (or avoid `$ref` to them).
  - BFF should include **components.securitySchemes** and **security** if the BFF is protected.
- Extending bff-generator (metadata for `components`, `security`) reduces the need for a post-processing step and keeps BRRTRouter's existing `security` and `parameter` logic valid.

---

## 12. Summary Table

| Category | Implemented | Partial | Not Implemented |
|----------|-------------|---------|-----------------|
| **Root** | info (title, version, description, contact), servers, paths, security, tags | — | webhooks, jsonSchemaDialect, externalDocs |
| **Components** | schemas, parameters, securitySchemes | examples (via oas3?) | responses, requestBodies, headers, examples (full), links, callbacks, pathItems |
| **Paths/Ops** | path, methods, parameters, requestBody, responses, security, deprecated (parsed) | Path Item $ref (if oas3 resolves) | path/op servers, callbacks, externalDocs |
| **Params** | path, query, header, cookie; schema; style; explode; $ref to components.parameters | — | content-based, allowReserved, allowEmptyValue, deprecated, content |
| **Schema** | type, format, properties, items, required, allOf, oneOf, anyOf, $ref, 3.1 `type: [T,null]` | nullable (legacy), example vs examples, discriminator | $schema, jsonSchemaDialect, contentEncoding, contentMediaType, xml |
| **Response** | status, content, schema, example/examples | — | headers, links, $ref to components.responses |
