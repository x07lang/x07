import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
import zlib
out=str(zlib.crc32(data)&0xffffffff).encode()
sys.stdout.buffer.write(out)
