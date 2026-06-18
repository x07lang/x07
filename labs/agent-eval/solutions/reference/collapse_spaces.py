import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
import re
out=re.sub(b' +', b' ', data)
sys.stdout.buffer.write(out)
