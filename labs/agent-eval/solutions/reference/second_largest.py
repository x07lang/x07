import sys
values = sorted(set(int(t) for t in sys.stdin.read().split()))
sys.stdout.write(str(values[-2]))
