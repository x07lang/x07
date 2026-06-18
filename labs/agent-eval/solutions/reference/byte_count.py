import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
out=str(len(data)).encode()
sys.stdout.buffer.write(out)
