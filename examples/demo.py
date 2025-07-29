#!/usr/bin/env python3
import time
import sys

print("Demo process started!")
counter = 0

try:
    while True:
        counter += 1
        print(f"Running... iteration {counter}")
        time.sleep(2)
except KeyboardInterrupt:
    print("Process interrupted, shutting down...")
    sys.exit(0)