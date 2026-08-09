# Response compression (Epic 13.8)

Opt-in **gzip** for eligible response bodies. Default is **off** so hot paths
do not pay surprise CPU.

## Enable

```yaml
compression:
  enabled: true
  min_bytes: 256   # optional; default 256
  level: 6         # optional; gzip 0–9
```

Clients must send `Accept-Encoding: gzip` (non-zero `q`).

## Eligibility

Compressed when all of:

- Config enabled
- Status `200` or `201`
- No existing `Content-Encoding`
- Content-Type is JSON / `text/*` / `+json` / XML-ish (not `text/event-stream`)
- Uncompressed size ≥ `min_bytes`
- Gzip shrinks the payload

Never compressed: SSE (`text/event-stream`), `image/*`, audio/video, zip/gzip,
`application/octet-stream`, already-encoded bodies.

## Wire format

Compressed bodies use the internal raw-body path (`x-brrtrouter-raw-encoding:
base64`) so `may_minihttp` emits a correct `Content-Length` over the gzip octets.
Clients see `Content-Encoding: gzip` and `Vary: Accept-Encoding`.

## Metrics

`brrtrouter_compression_responses_total` — count of gzipped responses.

## Failure policy

If gzip fails, the identity body is left unchanged (no truncated payload).
