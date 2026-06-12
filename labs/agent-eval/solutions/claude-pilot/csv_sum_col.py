import sys
total = 0
for line in sys.stdin.buffer.read().split(b"\n"):
    if line.strip():
        total += int(line.split(b",")[1])
sys.stdout.write(str(total))
