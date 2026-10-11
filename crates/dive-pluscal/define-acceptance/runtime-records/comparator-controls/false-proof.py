#!/usr/bin/env python3
import json,sys
from pathlib import Path
d=json.loads(Path('/Users/jeff/code/dive-pluscal-definitions-artifacts/comparator-controls/false-proof.json').read_text())
if 'false-proof' != "wrong-profile":d["profile"]="tlc"if "--compat=tlc"in sys.argv else"dive"
print(json.dumps(d))
