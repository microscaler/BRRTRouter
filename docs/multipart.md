# Multipart form-data (Stories 12.6 + 13.4)

BRRTRouter parses `multipart/form-data` into a JSON-compatible object for
validation and typed handlers. There is **no** silent empty-object bypass.

## Policy (MVP A — default)

| Part kind | Result |
|-----------|--------|
| Text field (`name=` only) | JSON string / loose scalar (`true`/`false`/numbers) |
| File field (`filename=` present) | Object: `filename`, `content_type`, `size`, `encoding` (`utf8`\|`omit`), optional `content` |
| Missing `boundary=` | **400** `reason: multipart_missing_boundary` |
| Malformed body | **400** `reason: multipart_malformed` |
| File part over 1 MiB (default) | **413** `reason: multipart_file_too_large` |

Binary file bytes are **omitted** from JSON (`encoding: "omit"`) but `size` /
`filename` remain so required-field checks can see the part. UTF-8 text files
under the cap include `content`.

Declared `request_content_types` still enforce **415** when the client sends a
type the operation does not list (e.g. multipart against JSON-only).

## Streaming file parts (Epic 13.4)

Opt-in: set `BRRTR_MULTIPART_STREAM_FILES=1` (or `true` / `yes`) so
`parse_request` streams file parts to disk instead of buffering content in JSON.

| Part kind | Result |
|-----------|--------|
| Text field | Same as MVP A |
| File field | `{ filename, content_type, size, encoding: "file", path }` |

- Default max streamed part: **64 MiB** (`DEFAULT_MAX_STREAMED_FILE_PART_BYTES`).
- Temp dir: `$TMPDIR/brrtrouter-uploads` (mode `0700` on Unix).
- On parse error, temp files from that attempt are deleted.
- **Ownership:** after a successful parse, the handler owns `path` and should
  delete it (or wrap with `TempUpload`, which deletes on drop unless `persist()`).

Library API (always available):

```rust
use brrtrouter::server::{
    parse_multipart_form_data_streaming, MultipartStreamOptions, TempUpload,
};

let body = parse_multipart_form_data_streaming(raw, content_type, &MultipartStreamOptions::default())?;
// body["file"]["path"] → filesystem path
```

Still honors Story 12.2 global/route body limits on the inbound request before
multipart parse runs.

## Downloads (Epic 13.4)

Typed handlers can return `brrtrouter::typed::HttpFile`:

```rust
use brrtrouter::typed::HttpFile;

HttpFile::attachment("report.pdf", "application/pdf", bytes)
// → Content-Disposition: attachment; filename="report.pdf"
// → Content-Type: application/pdf
// → raw octets on the wire (not JSON-schema-validated)
```

## OpenAPI `encoding` object

**Supported subset (documented):** not fully implemented. Multipart request
parsing uses `multipart/form-data` + part headers only. Per-property
`encoding` / `style` / `explode` from OpenAPI are **ignored** for now (same as
12.6). Track fuller fidelity under Epic 15 if needed.

## API

- `brrtrouter::server::parse_multipart_form_data` — MVP A (buffered)
- `brrtrouter::server::parse_multipart_form_data_streaming` — stream files to disk
- `brrtrouter::server::TempUpload` — RAII cleanup
- `brrtrouter::typed::HttpFile` — download helper

Suite narrative: Photon keeps product messaging; this file is the operator truth.
