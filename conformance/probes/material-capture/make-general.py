"""Supplement original16 selection with genuine sparse-domain function pairs."""
from pathlib import Path
import json
b=Path(__file__).resolve().parent;rows=[]
for ending in ['Fails','Holds']:
 n='MatGeneralFunction'+ending;p=b/'models'/n;p.mkdir()
 next="~x"if ending=='Holds'else'x'
 (p/(n+'.tla')).write_text(f'''---- MODULE {n} ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = FALSE
Next == \\E v \\in {{[i \\in {{1,3}} |-> i]}} : v[3] = 3 /\\ x' = {next}
Spec == Init /\\ [][Next]_x /\\ WF_x(Next)
Live == []<>(x = TRUE)
====
''');(p/(n+'.cfg')).write_text('SPECIFICATION Spec\nPROPERTY Live\n');rows.append(dict(name=n,kind='GeneralFunction',expected_exit=0 if ending=='Holds'else 13))
(b/'selection-general.json').write_text(json.dumps(rows,indent=2)+'\n')
