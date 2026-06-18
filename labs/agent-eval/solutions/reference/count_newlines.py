import sys
sys.stdout.write(str(sys.stdin.buffer.read().count(b"\n")))
