import struct
import sys
data = sys.stdin.buffer.read()
if data:
    lines = data.split(b"\n")
    if data.endswith(b"\n"):
        lines.pop()
    for line in lines:
        sys.stdout.buffer.write(struct.pack("<I", len(line)) + line)
