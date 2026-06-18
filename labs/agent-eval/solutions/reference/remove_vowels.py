import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
out=bytes(c for c in data if c not in b'aeiouAEIOU')
sys.stdout.buffer.write(out)
