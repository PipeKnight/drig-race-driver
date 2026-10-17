"""Print printable ASCII strings in a file whose text matches a regex, with file offsets.

Usage: python -I tools/scan_strings.py FILE REGEX [MINLEN]
"""
import re
import sys

data = open(sys.argv[1], "rb").read()
pattern = re.compile(sys.argv[2], re.I)
minlen = int(sys.argv[3]) if len(sys.argv) > 3 else 4
for m in re.finditer(rb"[\x20-\x7e]{%d,}" % minlen, data):
    s = m.group().decode("ascii")
    if pattern.search(s):
        print(f"{m.start():08x}\t{s}")
