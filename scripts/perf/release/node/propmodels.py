#!/usr/bin/env python3
"""propmodels.py <selection.txt>: models whose cfg has a PROPERTY/PROPERTIES entry."""
import glob
import os
import re
import sys

ex = glob.glob(os.path.expanduser("~/.cache/dive/" + "g" + "it/examples/*/"))[0]
for m in open(sys.argv[1]).read().split():
    text = re.sub(r"\\\*[^\n]*", "", open(os.path.join(ex, m)).read())
    text = re.sub(r"\(\*.*?\*\)", "", text, flags=re.S)
    if re.search(r"^\s*PROPERT(Y|IES)\b", text, flags=re.M):
        print(m)
