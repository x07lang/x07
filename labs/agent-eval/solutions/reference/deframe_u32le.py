import sys, base64, zlib, json, re
from collections import Counter
data = sys.stdin.buffer.read()
out=bytearray(); i=0
while i<len(data):
    L=int.from_bytes(data[i:i+4],'little'); i+=4
    out+=data[i:i+L]; i+=L; out+=b'\n'
out=bytes(out)
sys.stdout.buffer.write(out)
