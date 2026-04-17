import glob
import re


def process_file(path):
    if "dispatcher/core.rs" in path:
        return
    with open(path, "r") as f:
        content = f.read()

    # Find `HandlerRequest { ... reply_tx, ... }` precisely
    def replacer(match):
        inner = match.group(2)
        if "queue_guard" in inner:
            return match.group(0)  # Already added
        if (
            "reply_tx:" in inner
            or "reply_tx," in inner
            or inner.strip().endswith("reply_tx")
        ):
            # Ensure proper comma insertion
            if not inner.rstrip().endswith(","):
                inner = inner + ","
            return (
                match.group(1)
                + inner
                + "\n            queue_guard: None,"
                + match.group(3)
            )
        return match.group(0)

    new_content = re.sub(
        r"(HandlerRequest\s*\{)(.*?)(\s*\})", replacer, content, flags=re.DOTALL
    )

    if new_content != content:
        print(f"Fixed {path}")
        with open(path, "w") as f:
            f.write(new_content)


for p in glob.glob("src/**/*.rs", recursive=True):
    process_file(p)

for p in glob.glob("tests/**/*.rs", recursive=True):
    process_file(p)
