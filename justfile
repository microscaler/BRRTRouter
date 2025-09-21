# Tasks for local development

# default list of tasks
default:
	@just --list

# Build the pet store example
gen:
	cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# Force-regenerate the pet store example (explicit target)
gen-force:
	cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# Run the CLI generate command for any spec
generate spec="examples/openapi.yaml" force="":
	cargo run --bin brrtrouter-gen -- generate --spec {{spec}} {{force}}

# Serve a spec with echo handlers
serve spec="examples/openapi.yaml" addr="0.0.0.0:8080":
	cargo run --bin brrtrouter-gen -- serve --spec {{spec}} --addr {{addr}}

# Serve a spec and watch for changes
watch spec="examples/openapi.yaml" addr="0.0.0.0:8080":
	cargo run --bin brrtrouter-gen -- serve --watch --spec {{spec}} --addr {{addr}}

# Run tests with output
build:
	cargo build
	cargo build --release

# Run tests with output
test:
	cargo test -- --nocapture

# Measure code coverage (requires cargo-llvm-cov)
coverage:
	cargo llvm-cov --no-report

# Run benchmarks
bench:
	cargo bench

# Profile the example server with cargo flamegraph
fg:
	cargo flamegraph -p pet_store --bin pet_store

curls-start:
	# Start example with test key and wait a moment
	BRRTR_API_KEY=test123 cargo run --manifest-path examples/pet_store/Cargo.toml -- --spec doc/openapi.yaml &
	sleep 1
	just curls api="http://0.0.0.0:8080" key="test123"

curls api="http://0.0.0.0:8080" key="test123":
	# Infra
	curl -i {{api}}/health
	echo ""
	curl -i {{api}}/metrics
	echo ""

	# Pets
	curl -i -H "Authorization: {{key}}" "{{api}}/pets"
	echo ""
	curl -i -H "Authorization: {{key}}" -H "Content-Type: application/json" -d '{"name":"Bella"}' "{{api}}/pets"
	echo ""
	curl -i -H "Authorization: {{key}}" "{{api}}/pets/12345"
	echo ""

	# Users
	curl -i -H "Authorization: {{key}}" "{{api}}/users?limit=10&offset=0"
	echo ""
	curl -i -H "Authorization: {{key}}" "{{api}}/users/abc-123"
	echo ""
	curl -I -H "Authorization: {{key}}" "{{api}}/users/abc-123"   # HEAD
	echo ""
	curl -i -X OPTIONS -H "Authorization: {{key}}" "{{api}}/users/abc-123"
	echo ""
	curl -i -X DELETE -H "Authorization: {{key}}" "{{api}}/users/abc-123"
	echo ""

	# Posts
	curl -i -H "Authorization: {{key}}" "{{api}}/users/abc-123/posts?limit=5&offset=0"
	echo ""
	curl -i -H "Authorization: {{key}}" "{{api}}/users/abc-123/posts/post1"
	echo ""

	# Admin
	curl -i -H "Authorization: {{key}}" "{{api}}/admin/settings"
	echo ""

	# Items
	curl -i -H "Authorization: {{key}}" "{{api}}/items/550e8400-e29b-41d4-a716-446655440000"
	echo ""
	curl -i -H "Authorization: {{key}}" -H "Content-Type: application/json" -X POST -d '{"name":"New Item"}' "{{api}}/items/550e8400-e29b-41d4-a716-446655440000"
	echo ""

	# SSE (will hang; fetch headers only)
	curl -sS -m 2 -H "Authorization: {{key}}" "{{api}}/events" | head -n 1 || true
	echo ""

	# Download (JSON meta variant)
	curl -i -H "Authorization: {{key}}" "{{api}}/download/550e8400-e29b-41d4-a716-446655440000"
	echo ""

	# Form URL-encoded
	curl -i -H "Authorization: {{key}}" -H "Content-Type: application/x-www-form-urlencoded" --data-urlencode "name=John" --data-urlencode "age=30" "{{api}}/form"
	echo ""

	# Upload multipart (use a local file as payload)
	curl -i -H "Authorization: {{key}}" -F "file=@Cargo.toml;type=text/plain" -F "metadata.note=example" "{{api}}/upload"
	echo ""

	# Matrix and label style
	curl -i -H "Authorization: {{key}}" "{{api}}/matrix/;coords=1,2,3"
	echo ""
	curl -i -H "Authorization: {{key}}" "{{api}}/labels/.red"
	echo ""

	# Search with complex query, header and cookie
	curl -i -H "Authorization: {{key}}" -H "X-Trace-Id: 123e4567-e89b-12d3-a456-426614174000" --cookie "session=abc" \
	  "{{api}}/search?tags=a%7Cb%7Cc&filters%5Bname%5D=Bella&filters%5Btag%5D=pet"
	echo ""

	# Secure endpoint (requires API key)
	curl -i -H "Authorization: {{key}}" "{{api}}/secure"
	echo ""

	# Webhook registration
	curl -i -H "Authorization: {{key}}" -H "Content-Type: application/json" -d '{"url":"https://example.com/webhook"}' "{{api}}/webhooks"
	echo ""

all: gen build test curls

# Run nextest for faster test execution
nextest-test:
	cargo nextest run --workspace --all-targets --fail-fast --retries 1

alias nt := nextest-test
