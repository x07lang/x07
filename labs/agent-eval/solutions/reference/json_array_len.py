import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
import json
out=str(len(json.loads(data.decode()))).encode()
sys.stdout.buffer.write(out)
