import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
out=bytes.fromhex(data.decode())
sys.stdout.buffer.write(out)
