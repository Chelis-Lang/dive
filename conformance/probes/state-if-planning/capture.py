"""Capture the complete pinned conditional planning selection before native edits."""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path

p=argparse.ArgumentParser()
for name in ['java','javac','jar','reason']:p.add_argument('--'+name,required=True)
a=p.parse_args();root=Path(__file__).resolve().parent
pin='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
sha=lambda data:hashlib.sha256(data).hexdigest()
assert sha(Path(a.jar).read_bytes())==pin
if (root/'capture.json').exists():raise SystemExit('existing capture: retain it before regenerating')
cases=[
 ('StateGuardShortFalse','IF (FALSE /\\ x = 1) THEN 1 \\div 0 = 0 ELSE x = 2'),
 ('StateGuardShortTrue','IF (TRUE \\/ x = 1) THEN x = 2 ELSE 1 \\div 0 = 0'),
 ('StateGuardNegated','IF ~(x = 1 /\\ 1 \\div 0 = 0) THEN x = 2 ELSE x = 0'),
 ('StateGuardImplies','IF (x = 1 => 1 \\div 0 = 0) THEN x = 2 ELSE x = 0'),
 ('StateGuardEquiv','IF (x = 1 <=> 1 \\div 0 = 0) THEN x = 2 ELSE x = 0'),
 ('TemporalGuard','IF [](x = 1) THEN <>(x = 2) ELSE <>(x = 0)'),
 ('StateThenError','IF x = 1 THEN 1 \\div 0 = 0 ELSE x = 2'),
 ('StateElseError','IF x = 1 THEN x = 2 ELSE 1 \\div 0 = 0'),
 ('StateThenFalse','IF x = 1 THEN FALSE ELSE x = 2'),
 ('StateValid','IF x = 1 THEN x = 2 ELSE x = 0'),
 ('ConstantSkipThen','IF FALSE THEN 1 \\div 0 = 0 ELSE x = 2'),
 ('ConstantSkipElse','IF TRUE THEN x = 2 ELSE 1 \\div 0 = 0'),
 ('ConstantChosenThen','IF TRUE THEN 1 \\div 0 = 0 ELSE x = 2'),
 ('ConstantChosenElse','IF FALSE THEN x = 2 ELSE 1 \\div 0 = 0'),
 ('StateBothErrors','IF x = 1 THEN 1 \\div 0 = 0 ELSE 1 % 0 = 0'),
 ('ConstantBadGuard','IF 1 THEN x = 1 ELSE x = 2'),
 ('StateNonBoolThen','IF x = 1 THEN 42 ELSE x = 2'),
 ('StateNonBoolElse','IF x = 1 THEN x = 2 ELSE 42'),
 ('NestedState','IF x = 1 THEN (IF x = 0 THEN 1 \\div 0 = 0 ELSE x = 2) ELSE x = 0'),
 ('LetState','LET B == 1 \\div 0 = 0 IN IF x = 1 THEN B ELSE x = 2'),
 ('CallState','F(x = 1)'),
 ('ActionThenError',"IF x' = 1 THEN 1 \\div 0 = 0 ELSE x = 2"),
 ('ActionValid',"IF x' = 1 THEN x = 2 ELSE x = 0"),
 ('StateGuardError','IF x = 1 /\\ 1 \\div 0 = 0 THEN x = 2 ELSE x = 0'),
]
record={'schema_version':1,'oracle_sha256':pin,'source_commit':'94d0c5070852b37b801bb8998c436383ab7ee37b','reason':a.reason,'scope':'actual planning only; no state-space or verdict credit','rows':[]}
with tempfile.TemporaryDirectory(prefix='dive-state-if-') as temp:
 r=subprocess.run([a.javac,'-cp',a.jar,'-d',temp,str(root/'PlanningProbe.java')],capture_output=True)
 for channel in ['stdout','stderr']:(root/('compiler.'+channel)).write_bytes(getattr(r,channel))
 assert r.returncode==0, r.stderr.decode(errors='replace')
 for name,expression in cases:
  directory=root/'models'/name;directory.mkdir(parents=True,exist_ok=True)
  source=f'---- MODULE {name} ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x\\\' = (x + 1) % 3\nF(c) == IF c THEN 1 \\div 0 = 0 ELSE x = 2\nProp == []<>({expression})\n====\n'
  source=source.replace("x\\'", "x'")
  (directory/(name+'.tla')).write_text(source)
  (directory/(name+'.cfg')).write_text('INIT Init\nNEXT Next\nPROPERTY Prop\n')
  command=[a.java,'-Xmx256m','-cp',temp+':'+a.jar,'tlc2.tool.liveness.PlanningProbe',name]
  r=subprocess.run(command,cwd=directory,capture_output=True,timeout=15)
  raw=root/'raw'/name;raw.mkdir(parents=True,exist_ok=True)
  for channel in ['stdout','stderr']:(raw/channel).write_bytes(getattr(r,channel))
  fields=dict(line.split('\t',1) for line in r.stdout.decode().splitlines())
  record['rows'].append({'id':name,'expression':expression,'exit':r.returncode,'fields':fields})
  assert r.returncode==0 and 'completion' in fields, name
record['source_sha256']={str(path.relative_to(root)):sha(path.read_bytes()) for path in [root/'PlanningProbe.java',root/'capture.py',*sorted((root/'models').rglob('*.*'))]}
record['raw_sha256']={str(path.relative_to(root)):sha(path.read_bytes()) for path in [root/'compiler.stdout',root/'compiler.stderr',*sorted((root/'raw').rglob('stdout')),*sorted((root/'raw').rglob('stderr'))]}
(root/'capture.json').write_text(json.dumps(record,indent=2)+'\n')
print([(r['id'],r['fields']) for r in record['rows']])
