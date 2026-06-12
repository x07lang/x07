import re
import sys
from collections import Counter

words = re.findall(rb"[A-Za-z]+", sys.stdin.buffer.read())
counts = Counter(w.lower() for w in words)
best_count = max(counts.values())
best = min(w for w, c in counts.items() if c == best_count)
sys.stdout.write(f"{best.decode()} {best_count}")
