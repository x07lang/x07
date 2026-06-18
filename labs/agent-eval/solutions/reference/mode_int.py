import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
xs=[int(x) for x in data.split()]
c=Counter(xs)
out=str(max(c, key=lambda k:(c[k],-k))).encode()
sys.stdout.buffer.write(out)
