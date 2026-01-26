# BRRTRouter Python Validator

Python bindings for BRRTRouter's OpenAPI specification validation.

This module allows Python code to validate OpenAPI YAML/JSON files using the same validation logic that BRRTRouter uses before generating controllers/handlers.

## Status

🚧 **In Development** - This is a work in progress.

## Features

- Validate OpenAPI 3.1 specifications from files
- Validate OpenAPI specifications from YAML/JSON strings
- Detailed error reporting with locations
- Same validation logic as BRRTRouter

## Building

### Prerequisites

- Rust toolchain (1.70+)
- Python 3.9+ (for testing)
- PyO3 dependencies

### Build

```bash
cd brrtrouter-validator-python
cargo build --release
```

### Build Python Wheel

Using `maturin`:

```bash
pip install maturin
maturin build --release
```

## Usage (Python)

```python
from brrtrouter_validator import (
    validate_openapi_spec,
    validate_openapi_content,
    ValidationResult,
    ValidationError
)

# Validate from file
result = validate_openapi_spec("openapi.yaml")
if result.valid:
    print("✅ Valid OpenAPI spec")
else:
    for error in result.errors:
        print(f"❌ {error.location}: {error.message}")

# Validate from string
yaml_content = """
openapi: 3.1.0
info:
  title: Test API
  version: 1.0.0
paths:
  /test:
    get:
      operationId: testOperation
      responses:
        '200':
          description: Success
"""

result = validate_openapi_content(yaml_content, format="yaml")
if not result.valid:
    for error in result.errors:
        print(f"Error: {error.message}")
```

## API Reference

### `validate_openapi_spec(spec_path: str) -> ValidationResult`

Validate an OpenAPI specification file.

**Arguments:**
- `spec_path`: Path to the OpenAPI YAML or JSON file

**Returns:**
- `ValidationResult`: Object with `valid` (bool) and `errors` (list of `ValidationError`)

### `validate_openapi_content(content: str, format: str) -> ValidationResult`

Validate OpenAPI specification content from a string.

**Arguments:**
- `content`: OpenAPI specification content as a string
- `format`: Format of the content ("yaml", "yml", or "json")

**Returns:**
- `ValidationResult`: Object with `valid` (bool) and `errors` (list of `ValidationError`)

### `ValidationResult`

Result of OpenAPI validation.

**Attributes:**
- `valid` (bool): Whether the specification is valid
- `errors` (list[ValidationError]): List of validation errors (empty if valid)

**Methods:**
- `to_dict()`: Convert to Python dictionary

### `ValidationError`

A single validation error.

**Attributes:**
- `location` (str): Location in the spec where the error occurred
- `message` (str): Human-readable error message
- `kind` (str): Type of validation error

## Testing

```bash
cargo test --package brrtrouter-validator-python
```

## Integration with bff-generator

This module is designed to be integrated into the `bff-generator` Python tool to provide comprehensive OpenAPI validation before BFF spec generation.

See `../bff-generator/docs/BRRTRouter_VALIDATION_INTEGRATION.md` for integration details.

## License

MIT OR Apache-2.0 (same as BRRTRouter)
