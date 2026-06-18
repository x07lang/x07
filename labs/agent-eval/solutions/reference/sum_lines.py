import sys
total = sum(int(line) for line in sys.stdin.buffer.read().split(b"\n") if line.strip())
sys.stdout.write(str(total))
