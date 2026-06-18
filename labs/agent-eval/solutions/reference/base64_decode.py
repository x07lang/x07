import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
import base64
out=base64.b64decode(data)
sys.stdout.buffer.write(out)
