"""brrtrouter-tooling is a framework: no consuming project is named in its source.

Project names belong in the project's own ``brrtrouter.toml`` (``workspace.profile``). The only exceptions are the
deprecation shims for names that predate profiles: ``workspace/legacy_env.py`` and lines marked ``# legacy-name``.
"""

import re
from pathlib import Path

SRC = Path(__file__).resolve().parents[1] / "src" / "brrtrouter_tooling"
NAMES = re.compile(r"hauliage|rerp|pricewhisperer|sesame", re.IGNORECASE)
ALLOWED_FILES = {"legacy_env.py"}


def test_no_project_names_in_tooling_source() -> None:
    hits = []
    for path in sorted(SRC.rglob("*")):
        if not path.is_file() or "__pycache__" in path.parts or path.name in ALLOWED_FILES:
            continue
        if path.suffix in {".pyc", ".so"}:
            continue
        try:
            text = path.read_text()
        except UnicodeDecodeError:
            continue
        for n, line in enumerate(text.splitlines(), 1):
            if NAMES.search(line) and "# legacy-name" not in line:
                hits.append(f"{path.relative_to(SRC)}:{n}: {line.strip()}")
    assert not hits, "project names in tooling source (use the project profile):\n" + "\n".join(
        hits
    )
