import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
xs=[int(x) for x in data.split()]
c=Counter(xs)
out=(''.join('%d:%d\n'%(k,c[k]) for k in sorted(c))).encode()
sys.stdout.buffer.write(out)
