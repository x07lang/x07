import sys
h = 2166136261
for b in sys.stdin.buffer.read():
    h = ((h ^ b) * 16777619) & 0xFFFFFFFF
sys.stdout.write(str(h))
