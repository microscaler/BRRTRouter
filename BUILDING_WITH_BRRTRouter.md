# Building Microservices with BRRTRouter

This comprehensive guide defines the formal architectural layout for a microservice implemented natively behind `BRRTRouter`. It serves as the baseline for agents and third-party developers bootstrapping their own API services utilizing the OpenAPI generation engine alongside the `Lifeguard` ORM.

## Directory Overview 

A standard microservice directory tree (e.g., `hauliage/microservices/{your_service}`) looks strictly like this:

```
microservices/{service_name}/
├── gen/                 # ⚠️ Auto-generated. Do NOT edit.
│   ├── doc/
│   │   └── openapi.yaml # Auto-copied OpenAPI definition
│   └── src/
│       ├── handlers/    # Internal proxy routes mapping requests
│       ├── controllers/ # Scaffolded mock stubs natively returning `vec![]`
│       └── registry.rs  # The dispatcher macro `register_from_spec`
├── impl/                # 🛠️ Your Workspace!
│   ├── Cargo.toml       
│   ├── build.rs         
│   └── src/
│       ├── main.rs      # App entrypoint & routing logic
│       ├── controllers/ # User-owned HTTP handlers
│       ├── models/      # Lifeguard Database Entities 
│       ├── services/    # Core business logic / DB operations
│       └── validators/  # Request parameter validation
```

---

## The Generation Boundary (`gen/` vs `impl/`)

### What is Generated?
When `build.rs` fires, the `brrtrouter-gen` engine traverses your target OpenAPI specification and generates the entirety of the `gen/` directory instantly. It creates robust routing configurations and auto-scaffolds **Mock Stubs** inside `gen/.../controllers` representing every path defined in the YAML file. You never touch this folder.

### What is Implemented?
Your application physically lives inside the `impl/` directory. To replace a generation stub, you manually replicate the Rust function signature from `gen/src/controllers/{endpoint}.rs` and implement custom DB bindings natively inside `impl/src/controllers/`.

---

## 1. The OpenAPI Source Document
The absolute Source of Truth for the network boundary lives isolated at the repository root:  
**`openapi/{service_name}/openapi.yaml`**

Modifying paths or data constraints inside this specific YAML file dynamically triggers `brrtrouter-gen` to emit new struct mappings matching those exact fields upon deployment. 

### Generating the BFF (Backend-for-Frontend)
In multi-service architectures like Hauliage, the individual microservices do not face the external web payload natively. They are aggregated by a centralized proxy known as the BFF. 

To map a new microservice (or sync its updated schema) into the gateway:
1. **Declare the Schema Path:** Ensure your service is permanently scoped inside `openapi/bff-suite-config.yaml` with its physical port and local specification path.
    ```yaml
    services:
      your_service:
        base_path: /api/v1/your_service
        port: 8012
        spec_path: your_service/openapi.yaml
    ```
2. **Execute the Python `bff-generator`:** The network synthesizes the individual domain schemas into a monolithic aggregate using the standalone python `bff-generator` suite (`pip install bff-generator`).
    ```bash
    bff generate-system --config openapi/bff-suite-config.yaml
    ```
    Alternatively, inside your local `Tilt` workspace, the `bff-spec-gen` resource natively tracks the declarative config and auto-spools `openapi_bff.yaml` anytime you commit YAML changes.

3. **Deploy the Edge Node:** Once `openapi/openapi_bff.yaml` is synthesized by the Python generator, native `brrtrouter-gen` dynamically compiles the physical `microservices/bff` proxy server against the monolith template.

---

## 2. Models (`impl/src/models/`)
Here is where you define `Lifeguard` database traits representing the physical persistence rows natively tracking state constraints downstream.
- Structs strictly utilize the `#[derive(LifeModel)]` macro constraint.
- Data primitives must distinctly align perfectly with database primitives (e.g., PostgreSQL `UUID` columns must explicitly be bound to `pub id: uuid::Uuid` native Rust fields—failing to do so results in silent TypeErrors mapping into `[]`).
- These models do NOT interact with routing. They purely interface DDL constructs.

---

## 3. Services (`impl/src/services/`)
This is the core business logic layer executing native orchestration. 
- Services instantiate Database execution commands (`sea-query` wrappers acting over `Lifeguard`).
- Functions inside services explicitly strip HTTP primitives out, executing gracefully to handle logic, error matching, and safe transaction commits down-chain natively.
- Services consume raw models and cleanly yield serialized generic JSON payloads up onto the controllers natively.

---

## 4. Validators (`impl/src/validators/`)
Before your `service` explicitly interacts with HTTP traffic, controllers natively invoke `validators`. 
- They enforce logical execution boundaries not rigidly typed by the structural OpenAPI schema.
- For example, validating that payload `timestamps` don't physically precede `startOffset` boundaries before committing writes.

---

## 5. Migrations (`migrations/{service_name}/`)
Unlike models, Database physical migrations do natively live wholly stripped from the microservice workspace:
- **`lifeguard-migrate`** discovers your `impl/src/models/*.rs` structures natively during Tilt initialization loops. It automatically dumps completely precise `CREATE TABLE` and `ALTER TABLE` `.sql` schemas natively spanning here.
- 🔴 **NEVER manually edit `CREATE` or `ALTER` schemas generated here.** 
- 🟢 **DO manually place declarative `seed_...sql` injection scripts here.**

---

## 6. The Registration Pipeline (`impl/src/main.rs`)

To functionally bind your custom endpoint implementations natively up onto the underlying network, without `brrtrouter` completely discarding them during live-reloads, adopt **ADR 0001 (Register & Overwrite)** directly on startup:

```rust
// 1. Initialize ALL Auto-Stubs explicitly first
registry::register_from_spec(&mut dispatcher, &routes);

// 2. Iterate dynamically overriding stubs explicitly executed locally
for route in &routes {
    match route.handler_name.as_ref() {
        "your_implemented_endpoint" => {
            let tx = brrtrouter::typed::spawn_typed_with_stack_size_and_name(
                crate::controllers::your_implemented_endpoint::YourController,
                20480,
                Some(route.handler_name.as_ref()),
            );
            dispatcher.add_route(route.clone(), tx);
        }
        _ => {}
    }
}
```
