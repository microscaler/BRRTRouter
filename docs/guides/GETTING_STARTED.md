# Getting Started with BRRTRouter

> For people who have never touched BRRTRouter before — no prior knowledge of
> OpenAPI, coroutines, or this codebase assumed. By the end of this guide you
> will know **what problem BRRTRouter solves** and be able to **run the Pet
> Store demo** and poke at it with `curl` — with or without Kubernetes.

---

## 1. What this guide gives you

| Path | Time | What you get |
|------|------|--------------|
| **Path A — Run the demo locally** | ~10 min | Run the Pet Store example with plain `cargo`, call it with `curl`, change a handler, and see how the service is generated from an OpenAPI spec |
| **Path B — Full demo with observability** | ~30 min | The same Pet Store demo in a local Kubernetes cluster (kind + Tilt) with hot reload, a dashboard, Swagger UI, Prometheus, Grafana, and Jaeger |

**Prerequisites for Path A:** a Rust toolchain ([rustup](https://rustup.rs)) and `curl`.
**Path B additionally needs:** Docker, [kind](https://kind.sigs.k8s.io/), [kubectl](https://kubernetes.io/docs/tasks/tools/), and [Tilt](https://tilt.dev/). Full setup: [LOCAL_DEVELOPMENT.md](../LOCAL_DEVELOPMENT.md).

---

## 2. The problem BRRTRouter solves

Building a production microservice normally means writing a lot of glue that
isn't your business logic:

- Hand-written route tables and path-parameter extraction
- Request/response validation code
- Authentication and CORS wiring
- Observability bootstrap (metrics, tracing, health endpoints)
- curl scripts as your only "API documentation"

BRRTRouter **turns your OpenAPI 3.1 document into the source of truth**. You
describe the API once in `openapi.yaml`, and the generator produces a
type-safe Rust server that already has routing, JSON Schema validation,
security, CORS, metrics, and health checks wired in. You only fill in the
business logic.

| Traditional approach | With BRRTRouter |
| -------------------- | --------------- |
| Hand-written routes per endpoint | Routes + params compiled from OpenAPI |
| Ad-hoc validation | JSON Schema + required-parameter checks → automatic **400** |
| Bolt-on auth and CORS | `securitySchemes` + `config.yaml` + `x-cors` |
| Separate observability bootstrap | `/metrics`, OpenTelemetry, `/health` included |
| curl scripts as "docs" | Pet Store dashboard + Swagger UI |

It is built for **cloud-native scale-out**: on a 2-core CI runner (a typical
Kubernetes pod shape) it sustains **~1,500+ req/s at ~8 ms median latency with
0% failures** — see [PERFORMANCE.md](../PERFORMANCE.md) for the Goose load-test
numbers. For the business-level framing, see
[`docs/marketing/solution_brief.md`](../marketing/solution_brief.md).

---

## 3. Core concepts (3 minutes)

Three ideas carry the whole project:

**1. OpenAPI is the source of truth.**
Paths, methods, parameters, request/response schemas, and security are read
from your spec. Routing, validation, and generated code all derive from it.
Change the API? Edit the spec and regenerate — you never hand-write route
tables.

**2. Generated code vs. your code.**
The generator emits everything mechanical: route registry, request/response
types, middleware wiring. Your business logic lives in **controllers**, which
are plain Rust functions — here is the real `get_pet` controller from the Pet
Store example:

```rust
#[handler(GetPetController)]
pub fn handle(_req: TypedHandlerRequest<Request>) -> Response {
    Response {
        age: 3,
        breed: "Golden Retriever".to_string(),
        id: 12345,
        name: "Max".to_string(),
        tags: vec!["friendly".to_string(), "trained".to_string()],
        vaccinated: true,
    }
}
```

No `async`, no `await`, no futures — BRRTRouter runs your handlers on
**coroutines** (lightweight threads via the `may` runtime), so you write
ordinary synchronous Rust and the runtime handles concurrency. This is
unusual for a Rust web framework and is the main "beginner-friendly" trick:
see [`docs/marketing/BEGINNER_GUIDE.md`](../marketing/BEGINNER_GUIDE.md).

**3. The runtime pipeline.**
Each request flows through middleware (CORS → tracing → auth → metrics),
gets matched against the compiled route table, is dispatched into a handler
coroutine, and the response is validated and returned. Sequence diagram:
[RequestLifecycle.md](../RequestLifecycle.md). Concepts in depth:
[BRRTRouter_OVERVIEW.md](../BRRTRouter_OVERVIEW.md).

---

## 4. Path A — Run the demo locally (no Kubernetes)

`examples/pet_store` is a complete demo service generated from
`examples/pet_store/doc/openapi.yaml`. Run it directly with cargo — no
cluster needed.

### 4.1 Run it

```bash
cd examples/pet_store
cargo run
```

The first build compiles the BRRTRouter library and takes a few minutes;
later builds are incremental and fast. When you see
`🚀 pet_store example server listening on 0.0.0.0:8081`, the server is up.
(The demo listens on 8081 by default to avoid clashing with dev tooling on
8080.)

### 4.2 Call it

```bash
curl http://localhost:8081/health
# {"status":"ok"}

curl -H "X-API-Key: test123" http://localhost:8081/pets
# [{"age":3,"breed":"Golden Retriever","id":12345,"name":"Max",...}, ...]

curl -H "X-API-Key: test123" http://localhost:8081/pets/12345
# {"age":3,"breed":"Golden Retriever","id":12345,"name":"Max","tags":["friendly","trained"],"vaccinated":true}
```

`/health` is a liveness endpoint the generator added for free, and `/metrics`
exposes Prometheus counters the same way. Swagger UI is at
http://localhost:8081/docs, and the SolidJS dashboard is one flag away:

```bash
cargo run -- --static-dir ./static_site
# dashboard: http://localhost:8081/
```

The `X-API-Key` header is the "security for free" in action: the spec
declares an `ApiKeyHeader` security scheme and the key is configured in
`config/config.yaml`. Call `/pets` **without** the header and you get a 401
with a `problem+json` body — the generator wired that check in for you:

```bash
curl http://localhost:8081/pets
# {"detail":"Missing or invalid credentials","status":401,"title":"Unauthorized","type":"about:blank"}
```

If port 8081 is already taken on your machine, start it elsewhere:

```bash
PORT=18081 cargo run
curl -H "X-API-Key: test123" http://localhost:18081/pets
```

### 4.3 Change your first handler

Open `examples/pet_store/src/controllers/get_pet.rs` — this is **your** code.
Change the `name` field from `"Max"` to something else (there is a JSON
string near the top of the file as well — change it there too), restart the
server (Ctrl-C, `cargo run` again), and call `/pets/12345` again to see the
new response.

When you later regenerate from the spec, add the line
`// BRRTRouter: user-owned` to any controller you've implemented — the
generator never overwrites files carrying that marker, even with `--force`.

### 4.4 Look around the generated service

```
pet_store/
├── Cargo.toml            # workspace member of the BRRTRouter repo
├── doc/openapi.yaml      # the spec this service was generated from
├── config/config.yaml    # env-specific settings: security keys, CORS origins, http
├── static_site/          # optional static files
└── src/
    ├── main.rs           # server bootstrap (ports, config loading)
    ├── registry.rs       # generated: routes compiled from the spec
    ├── handlers/         # generated: request/response types per endpoint
    └── controllers/      # YOUR code: one file per operation
```

Two files matter for day-to-day work: the **spec** (change the API) and the
**controllers** (change the behavior). Everything else is generated.

### 4.5 Generate a service from your own spec

When you're ready to start from your own OpenAPI document instead of the
example, the generator does the work (run from the repo root):

```bash
# Lint your spec for BRRTRouter conformance first:
cargo run --bin brrtrouter-gen -- lint --spec examples/your_service.yaml

# Generate the server crate for a spec (output inside this repo, e.g. examples/):
cargo run --bin brrtrouter-gen -- generate \
  --spec examples/your_service.yaml \
  --output examples/your_service \
  --force
```

The `generate` step emits a complete standalone crate (like `pet_store`).
The CLI derives the path to the BRRTRouter library from the output location,
so outputs inside this repo work out of the box; for generating into a
**separate** repo, use `brrtrouter-tooling` instead, which resolves that path
explicitly (see [`tooling/README.md`](../../tooling/README.md)).

The `generate-stubs` command scaffolds controller stubs for the other layout —
the multi-service `gen/` + `impl/` split where the impl crate lives inside a
Cargo workspace — described in [BUILDING_MICROSERVICES.md](BUILDING_MICROSERVICES.md).

> ⚠️ If you generate a standalone crate **inside this repo's directory tree**
> (e.g. `--output examples/my_service`), the generator's final `cargo fmt`
> step fails with `current package believes it's in a workspace when it's
> not` — the repo root is a Cargo workspace. Fix: either add the crate to
> `workspace.members` in the root `Cargo.toml` (that is how `pet_store` is
> set up) or add an empty `[workspace]` table to the generated `Cargo.toml`,
> then run `cargo fmt` yourself.

### 4.6 Accessing the demo from another machine (VM / remote host)

Everything above uses `localhost` because you run the commands **on the same
machine** as the server. If BRRTRouter runs in a VM (or another host) and you
want to reach it from your own machine, note:

- **The server listens on all interfaces by default.** The startup line says
  `listening on 0.0.0.0:8081` — not `127.0.0.1` — so it is reachable from
  other machines on the network. (Setting `BRRTR_LOCAL=1` restricts it to
  loopback if you ever want the opposite.)
- **Replace `localhost` with the VM's address** in every example:

  ```bash
  curl http://192.168.1.50:8081/health                       # from your host
  curl -H "X-API-Key: test123" http://192.168.1.50:8081/pets
  # browser: http://192.168.1.50:8081/  (dashboard)  ·  /docs  (Swagger)
  ```

- **NAT networking (VirtualBox/VMware default) has no inbound access.**
  Either configure port forwarding in the VM manager (host port → guest
  8081), use bridged networking so the VM gets a LAN IP, or tunnel over SSH
  from your host and keep using `localhost`:

  ```bash
  ssh -L 8081:127.0.0.1:8081 user@vm-host   # then http://localhost:8081/ works on your host
  ```

- **The VM's firewall must allow the port** (e.g. `ufw allow 8081` on Ubuntu).
- **CORS is not a problem for the dashboard and Swagger** — they are served
  from the same origin (port 8081) as the API, so a browser on another
  machine can use them without changes. CORS matters only for *separate*
  frontends: `config/config.yaml` allows `http://localhost:3000` (the
  sample-ui dev server) by default; if you run such a UI on your own host,
  add its origin there, e.g. `http://192.168.1.20:3000`.

---

## 5. Path B — Full demo with observability (Tilt + kind)

The same Pet Store demo, now in a local Kubernetes cluster with hot reload,
a SolidJS dashboard, Swagger UI, Prometheus, Grafana, and Jaeger.

```bash
# 1. Bring up the shared kind cluster (sibling monorepo; one-time):
cd ../shared-kind-cluster && just dev-up

# 2. Bring up the demo in this repo:
cd - && just dev-up            # Tilt; watch the browser UI that opens
```

Once Tilt shows everything green:

```bash
curl http://localhost:8081/health
curl -H "X-API-Key: test123" http://localhost:8081/pets
```

- **Dashboard:** http://localhost:8081/
- **Swagger UI:** http://localhost:8081/docs
- Edit a controller or the spec and Tilt rebuilds/restarts the pod in seconds.

Tilt port-forwards to the machine running the cluster — if that is a VM and
you are on another host, apply the same substitution or SSH tunnel as in
[section 4.6](#46-accessing-the-demo-from-another-machine-vm--remote-host).

Setup details and troubleshooting: [LOCAL_DEVELOPMENT.md](../LOCAL_DEVELOPMENT.md),
[KIND_TROUBLESHOOTING.md](../KIND_TROUBLESHOOTING.md).

---

## 6. What you get for free

These come from the spec + config, no extra code:

| Feature | How it's activated |
|---------|--------------------|
| Request/response **JSON Schema validation** | Always on; invalid input → **400**, invalid output → **500** ([PROBLEM_DETAILS.md](../PROBLEM_DETAILS.md)) |
| **Security** (API keys, bearer, OAuth2, JWKS, PropelAuth) | `securitySchemes` in the spec + provider config in `config.yaml` |
| **CORS** | Global settings in `config.yaml` + per-route `x-cors` in the spec ([CORS.md](../CORS.md)) |
| **Metrics / health** | `/metrics` (Prometheus) and `/health` are served automatically |
| **Tracing / logs** | OpenTelemetry wired in; structured logs via `RUST_LOG` |
| **SSE streaming** | Add `x-sse` to an operation ([SSE_LIVE_FLUSH.md](../SSE_LIVE_FLUSH.md)) |
| **Static file serving** | Point `--static-dir` at a folder |

---

## 7. The example: Pet Store

`examples/pet_store/` is the demo for everything in this guide:

- **The spec** — `examples/pet_store/doc/openapi.yaml` defines the whole
  surface: CRUD endpoints, the `ApiKeyHeader` security scheme, CORS
  extensions. Read it alongside section 3 to see how "one spec" turns into
  routing + validation + auth.
- **The dashboard** — a SolidJS UI at http://localhost:8081/ (Path B).
- **Swagger UI** — http://localhost:8081/docs, generated from the same spec.
- **Load tests** — Goose scenarios exercise the demo end to end; see
  [GOOSE_LOAD_TESTING.md](../GOOSE_LOAD_TESTING.md).

---

## 8. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `Address already in use` on 8081 | Start with `PORT=18081 cargo run` (section 4.2) |
| `error: current package believes it's in a workspace when it's not` — when building an example, or when the generator's `cargo fmt` step fails with that message | The crate sits inside the repo workspace but isn't a member; add it to `workspace.members` in the root `Cargo.toml` (as `pet_store` is), or add an empty `[workspace]` table to its `Cargo.toml` |
| First build is slow | Expected — the BRRTRouter library compiles once, then it's cached |
| `just dev-up` fails / kind problems | [KIND_TROUBLESHOOTING.md](../KIND_TROUBLESHOOTING.md) |
| Curl returns `401` on Pet Store | You forgot the API key header: `-H "X-API-Key: test123"` |

---

## 9. Where to go next

- **Understand the concepts and architecture** → [BRRTRouter_OVERVIEW.md](../BRRTRouter_OVERVIEW.md), then [ARCHITECTURE.md](../ARCHITECTURE.md)
- **Build a real multi-service product** → [BUILDING_MICROSERVICES.md](BUILDING_MICROSERVICES.md) (the `gen/` / `impl/` layout and BFF gateway)
- **Develop in this repo** → [CONTRIBUTING.md](../../CONTRIBUTING.md), [DEVELOPMENT.md](../DEVELOPMENT.md)
- **Load-test your service** → [GOOSE_LOAD_TESTING.md](../GOOSE_LOAD_TESTING.md)
- **See a production-style reference** → [BUILDING_WITH_BRRTROUTER.md](../BUILDING_WITH_BRRTROUTER.md) (Sesame-IDAM)
- **Roadmap and epics** → [EPICS_CATALOG.md](../EPICS/EPICS_CATALOG.md)

Something unclear? That's a bug in this guide — please report it (see
[CONTRIBUTING.md](../../CONTRIBUTING.md) for how).
