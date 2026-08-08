# Multipart form-data (Story 12.6)

BRRTRouter parses `multipart/form-data` into a JSON-compatible object for
validation and typed handlers. There is **no** silent empty-object bypass.

## Policy (MVP A)

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

## Not in this story

- Stream-to-disk uploads / download helpers → [Epic 13.4](./EPICS/FRAMEWORK_MATURITY/epic-13-framework-completeness/story-13.4-streaming-uploads-downloads.md)
- Full OpenAPI `encoding` object / `style` matrix for multipart
- Streaming large uploads to disk
- Multipart response generation

## API

- `brrtrouter::server::parse_multipart_form_data`
- Wired through `parse_request` → `ParsedRequest.body`

Suite narrative: Photon keeps product messaging; this file is the operator truth.
