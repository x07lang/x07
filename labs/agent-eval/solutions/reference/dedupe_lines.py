import sys
data = sys.stdin.buffer.read()
if data:
    lines = data.split(b"\n")
    if data.endswith(b"\n"):
        lines.pop()
    seen = set()
    out = []
    for line in lines:
        if line not in seen:
            seen.add(line)
            out.append(line + b"\n")
    sys.stdout.buffer.write(b"".join(out))
