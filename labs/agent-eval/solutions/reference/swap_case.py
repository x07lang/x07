import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
out=bytes((c-32) if 97<=c<=122 else (c+32) if 65<=c<=90 else c for c in data)
sys.stdout.buffer.write(out)
