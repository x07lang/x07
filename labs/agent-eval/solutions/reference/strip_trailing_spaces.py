import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
s=data.split(b'\n')
if s and s[-1]==b'': s=s[:-1]
out=b''.join(l.rstrip(b' \t')+b'\n' for l in s)
sys.stdout.buffer.write(out)
