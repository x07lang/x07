import json
import sys
sys.stdout.write(json.load(sys.stdin)["name"])
