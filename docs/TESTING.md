# Testing with `TestApp` (Epic 13.10)

BRRTRouter exposes an in-process HTTP test client under the **`testing`** Cargo
feature so product crates (pet_store, Sesame, …) do not copy private
`tests/common` TCP helpers.

## Enable

```toml
# In your crate
[dev-dependencies]
brrtrouter = { path = "../BRRTRouter", features = ["testing"] }
```

```bash
cargo test --features testing
# or, from this repo:
just test   # passes --features testing
just nt
```

## Drive pet_store from its OpenAPI

Pet store’s surface is `examples/pet_store/doc/openapi.yaml` (kept aligned with
`examples/openapi.yaml` via `brrtrouter-gen`). **Gen** produces handlers/registry;
**TestApp** exercises the running service at test time.

```rust,ignore
use brrtrouter::test_support::TestApp;
use brrtrouter::server::AppService;
// … build AppService like pet_store main (load_spec_full + register_from_spec
//   + security providers), then:

let app = TestApp::from_service(service)?;
let res = app
    .get("/pets")
    .header("X-API-Key", "test123")
    .send()?;
assert_eq!(res.status, 200);
```

Or register via `from_spec` / `from_spec_with_options` when you do not need
pre-start service mutation:

```rust,ignore
use brrtrouter::test_support::{TestApp, TestAppOptions};

let app = TestApp::from_spec_with_options(
    "examples/pet_store/doc/openapi.yaml",
    TestAppOptions {
        static_dir: Some("examples/pet_store/static_site".into()),
        doc_dir: Some("examples/pet_store/doc".into()),
    },
    |dispatcher, routes| unsafe {
        pet_store::registry::register_from_spec(dispatcher, routes);
    },
)?;
```

## API sketch

| Type | Role |
|------|------|
| `TestApp::from_service` | Bind `127.0.0.1:0`, start, wait ready |
| `TestApp::from_spec` | Load OpenAPI + register handlers + start |
| `RequestBuilder` | `get`/`post`/`header`/`cookie`/`json`/`send` |
| `TestResponse` | `status`, `headers`, `body`, `json()`, `text()` |

Authorization header values are redacted in `RequestBuilder`’s `Debug` output.

## Non-goals

- Docker / curl E2E (`tests/common/pet_store_e2e.rs`) — still for container tests
- Browser automation or Goose load drivers
