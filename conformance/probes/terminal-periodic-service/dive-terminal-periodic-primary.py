from pathlib import Path
import subprocess,json,hashlib,time,os
b=Path('/tmp/dive-terminal-periodic-primary-candidate0');b.mkdir(exist_ok=False)
cache=Path('/Users/jeff/code/dive-periodic-snapshot-cache')
jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar'
java=cache/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
cases=[
 ('safe-absent','<>(x=1)',None,None,None),
 ('safe-true','<>(x=1)','TRUE',None,None),
 ('safe-false','<>(x=1)','FALSE',None,None),
 ('safe-integer','<>(x=1)','1',None,None),
 ('safe-division','<>(x=1)','1 \\div 0',None,None),
 ('safe-state','<>(x=1)','x=1',None,None),
 ('violating-absent','<>(x=2)',None,None,None),
 ('violating-true','<>(x=2)','TRUE',None,None),
 ('violating-false','<>(x=2)','FALSE',None,None),
 ('violating-error','<>(x=2)','1 \\div 0',None,None),
 ('init-error-absent','<>(x=1)',None,'x=(1 \\div 0)',None),
 ('init-error-false','<>(x=1)','FALSE','x=(1 \\div 0)',None),
 ('next-error-absent','<>(x=1)',None,None,"x'=(1 \\div 0)"),
 ('next-error-false','<>(x=1)','FALSE',None,"x'=(1 \\div 0)"),
 ('empty-init-absent','<>(x=1)',None,'FALSE',None),
 ('empty-init-false','<>(x=1)','FALSE','FALSE',None),
 ('no-actions-absent','<>(x=1)',None,None,'NO_ACTIONS'),
 ('no-actions-false','<>(x=1)','FALSE',None,'NO_ACTIONS'),
 ('missing-hook','<>(x=1)','MISSING',None,None),
 ('nonzero-hook','<>(x=1)','NONZERO',None,None),
]
selection=[]
for width in [1,4,8]:
 for name,prop,hook,init,next in cases:
  selection.append(dict(id=name+'-w'+str(width),width=width,case=name,property=prop,hook=hook,init=init,next=next))
(b/'selection.json').write_text(json.dumps(selection,indent=2)+'\n')
rows=[]
for i,row in enumerate(selection):
 d=b/row['id'];d.mkdir();(d/'meta').mkdir()
 hook=row['hook'];body=''if hook in [None,'MISSING']else ('Hook(v) == v\n'if hook=='NONZERO'else 'Hook == '+hook+'\n')
 source='---- MODULE Terminal ----\nEXTENDS Integers\nVARIABLE x\nInit == '+(row['init']or'x=0')+'\nNext == '+('UNCHANGED x'if row['next']=='NO_ACTIONS'else row['next']or"x'=(IF x=0 THEN 1 ELSE 1)")+'\nLive == '+row['property']+'\n'+body+'====\n'
 cfg='INIT Init\n'+(''if row['next']=='NO_ACTIONS'else'NEXT Next\n')+'PROPERTY Live\nCHECK_DEADLOCK FALSE\n'+(''if hook is None else '_PERIODIC '+('Missing'if hook=='MISSING'else'Hook')+'\n')
 (d/'Terminal.tla').write_text(source);(d/'Terminal.cfg').write_text(cfg)
 cmd=[str(java),'-cp',str(jar),'tlc2.TLC','-tool','-workers',str(row['width']),'-seed','0','-fp','0','-cleanup','-metadir',str(d/'meta'),'Terminal']
 start=time.monotonic();timeout=False
 with(d/'stdout').open('wb')as out,(d/'stderr').open('wb')as err:
  try:r=subprocess.run(cmd,cwd=d,stdout=out,stderr=err,timeout=30);exit=r.returncode
  except subprocess.TimeoutExpired:timeout=True;exit=None
 raw=(d/'stdout').read_text(errors='replace');codes=[]
 import re
 for m in re.finditer(r'@!@!@STARTMSG (\d+):(\d+)',raw):codes.append([int(m[1]),int(m[2])])
 record=dict(**row,command=cmd,exit=exit,timeout=timeout,wall_seconds=time.monotonic()-start,message_codes=codes,scope='Actual unmodified full TLC run; output does not prove internal queue/join/clock fields',artifact_sha256={str(p.relative_to(d)):sha(p)for p in d.rglob('*')if p.is_file()})
 rows.append(record);(b/'capture.json').write_text(json.dumps(dict(oracle_sha256=sha(jar),rows=rows,ordinary_native_proof=False),indent=2)+'\n')
 print(row['id'],exit,codes[-6:],flush=True)
assert len(rows)==60
print('Closed60 actual full TLC terminal selections; no native scheduling proof',flush=True)
