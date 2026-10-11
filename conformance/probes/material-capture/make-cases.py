"""Generate paired finite material-value admission fixtures before implementation."""
from pathlib import Path
import json
base=Path(__file__).resolve().parent
patterns=[('String','CONSTANT Data','Data = "token"','Data = "token"',None),('Model','CONSTANT Data','Data = token','Data = Data',None),('EnumSet','CONSTANT Data','Data = {1, 2}','1 \\in Data',None),('Tuple','','','v[1] = 1','<<1, 2>>'),('Record','','','v.a = 1','[a |-> 1, b |-> 2]'),('Function','','','v[1] = 1','[i \\in {1, 2} |-> i]'),('CapturedSet','','','1 \\in v','{1, 2}'),('Nested','CONSTANT a, b','a = a\nb = b','v[1].tag = "ok"','<<[tag |-> "ok", vals |-> {a, b}], <<1, 2>>>>')]
rows=[]
for kind,decl,cfg,predicate,value in patterns:
 for fail in [True,False]:
  name='Mat'+kind+('Fails' if fail else 'Holds');folder=base/'models'/name;folder.mkdir(parents=True,exist_ok=True)
  step='x' if fail else '~x';action=f"{predicate} /\\ x' = {step}"
  if value:action='\\E v \\in {'+value+'} : '+action
  init='x = FALSE'+(' /\\ '+predicate if value is None else '')
  (folder/(name+'.tla')).write_text(f'---- MODULE {name} ----\nEXTENDS Naturals, TLC\n{decl}\nVARIABLE x\nInit == {init}\nNext == {action}\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nLive == []<>(x = TRUE)\n====\n')
  (folder/(name+'.cfg')).write_text('SPECIFICATION Spec\nPROPERTY Live\n'+('CONSTANTS '+cfg+'\n' if cfg else ''))
  rows.append(dict(name=name,kind=kind,expected_exit=13 if fail else 0))
(base/'selection.json').write_text(json.dumps(rows,indent=2)+'\n')
print('Prepared',len(rows),'paired primary controls')
