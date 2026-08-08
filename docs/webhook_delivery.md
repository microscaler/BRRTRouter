# Outbound webhook delivery kit (Story 12.5)

Library helper for **Sesame-style** webhook delivery: HTTP POST of a JSON payload
with optional HMAC-SHA256 signature, bounded retries, and backoff.

**Not in scope:** OpenAPI Callback Object auto-fire, DLQ UI, inbound signature
verification middleware. Subscription CRUD stays normal OpenAPI paths.

## API

```rust
use brrtrouter::http::{
    deliver_webhook, WebhookDeliveryOptions, WebhookHmac,
};

let result = deliver_webhook(&WebhookDeliveryOptions {
    url: subscription_url,
    body: serde_json::to_vec(&payload)?,
    hmac: Some(WebhookHmac::required(shared_secret)),
    idempotency_key: Some(delivery_id),
    max_attempts: 3,
    ..WebhookDeliveryOptions::default()
})?;
```

| Option | Default | Notes |
|--------|---------|--------|
| `timeout` | 5s | Per-attempt (`HttpFetchOptions`) |
| `max_attempts` | 3 | Includes first try |
| `initial_backoff` | 50ms | Exponential (`×2` each retry), cooperative via `may::coroutine::sleep` |
| `max_request_body_bytes` | 1 MiB | Fail closed before connect |
| `max_response_body_bytes` | 64 KiB | Bound subscriber response |
| HMAC header | `X-Hub-Signature-256` | Value `sha256=<hex>` |
| Idempotency | optional | `Idempotency-Key` header |

### Retry policy

- **2xx** → success
- **4xx** → error immediately (not retried)
- **5xx / 408 / 429** and transport errors → retry until `max_attempts`

### Secrets

[`HmacSecret`](../src/http/webhook_delivery.rs) redacts in `Debug` / `Display`.
Do not log `expose()`.

## Sesame integration pattern

`org-mgmt` exposes OpenAPI operations such as `test_webhook_delivery` (CRUD for
subscriptions is separate). The **impl** controller should:

1. Load subscription URL (+ secret) from storage.
2. Build a small JSON test payload.
3. Call `deliver_webhook`.
4. Map [`WebhookDeliveryResult`] / [`WebhookDeliveryError`] into the typed response
   (`success`, `delivery_status`, `endpoint_url`, `message`).

Stub today:
`sesame-idam/microservices/idam/org-mgmt/impl/src/controllers/test_webhook_delivery.rs`

Outward suite narrative: Photon [`docs/webhooks.md`](https://github.com/microscaler/photon/blob/main/docs/webhooks.md).

## Tests

`tests/webhook_delivery_tests.rs` + in-module unit tests in
`src/http/webhook_delivery.rs` cover Story 12.5 P1–P6 / N1–N8.
