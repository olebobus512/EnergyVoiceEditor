"""Energy preset API v1: stdin JSON -> stdout JSON; no external dependencies."""
import json
import sys
request = json.load(sys.stdin)
assert request["apiVersion"] == 1
notes = [{"midi": [60, 64, 67, 72][i % 4], "start": i * 0.5,
          "duration": 0.45, "lyric": "la"} for i in range(32)]
json.dump({"name": "C major arpeggio", "tempo": 120, "notes": notes}, sys.stdout)
