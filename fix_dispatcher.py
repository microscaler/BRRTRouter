import re

with open("tests/dispatcher_tests.rs", "r") as f:
    content = f.read()

def replacer(match):
    inner = match.group(2)
    if "queue_guard" in inner: return match.group(0)
    if "reply_tx:" in inner or "reply_tx," in inner or inner.strip().endswith("reply_tx"):
        if not inner.rstrip().endswith(','):
            inner = inner + ','
        return match.group(1) + inner + '\n            queue_guard: None,' + match.group(3)
    return match.group(0)

new_content = re.sub(
    r'(HandlerRequest\s*\{)(.*?)(\s*\})',
    replacer,
    content,
    flags=re.DOTALL
)

with open("tests/dispatcher_tests.rs", "w") as f:
    f.write(new_content)
