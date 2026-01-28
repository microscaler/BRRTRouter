# BRRTRouter Tooling

Development tooling for BRRTRouter project automation.

## Installation

```bash
cd tooling
pip install -e ".[dev]"
```

## Usage

```bash
brrtrouter dependabot <command>
```

## Development

### Linting

```bash
ruff check src/ tests/
ruff format src/ tests/
```

### Testing

```bash
pytest
```

### Building

```bash
pip install -e .
```
