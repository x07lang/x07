import sys
data = sys.stdin.buffer.read()
if data:
    lines = data.split(b"\n")
    if data.endswith(b"\n"):
        lines.pop()
    sys.stdout.buffer.write(b"".join(line + b"\n" for line in reversed(lines)))
