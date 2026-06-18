import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
xs=[int(x) for x in data.split()]
s=0; parts=[]
for x in xs:
    s+=x; parts.append(str(s))
out=(''.join(p+'\n' for p in parts)).encode()
sys.stdout.buffer.write(out)
