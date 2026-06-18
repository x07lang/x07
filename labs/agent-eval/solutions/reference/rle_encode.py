import sys
from itertools import groupby

out = []
for byte, run in groupby(sys.stdin.buffer.read()):
    out.append(bytes([byte]) + str(sum(1 for _ in run)).encode())
sys.stdout.buffer.write(b"".join(out))
