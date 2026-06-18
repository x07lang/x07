import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
xs=[int(x) for x in data.split()]
out=(' '.join(str(x) for x in sorted(xs))).encode() if xs else b''
sys.stdout.buffer.write(out)
