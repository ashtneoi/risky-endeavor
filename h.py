#!/usr/bin/env python3
import os
import struct
import sys

assert not os.isatty(sys.stdout.fileno())

for line in sys.stdin:  # with newlines
    if len(line) < 9:
        continue
    x = line[0:9]
    if x[4] == "'":
        i = int(x[0:4] + x[5:9], 16)
        sys.stdout.buffer.write(struct.pack("<I", i))
