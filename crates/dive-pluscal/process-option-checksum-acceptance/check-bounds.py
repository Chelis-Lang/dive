#!/usr/bin/env python3
"""Full editor bounds, deterministic malformed placement and UTF-8 spans."""
import argparse,hashlib,json,random,subprocess
from pathlib import Path
BASE=Path(__file__).resolve().parent
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor',type=Path,required=True);a=p.parse_args();rng=random.Random(17247);source='---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable g=0; process P \in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)\n\\* λ🙂 footer\n====\n';rows=[]
for index in range(512):
 text=source
 if index%3==0:
  at=rng.randrange(len(text)+1);text=text[:at]+rng.choice(['(*','*)','\\* BEGIN TRANSLATION\n','🙂','\r\n','\0'])+text[at:]
 limit=rng.choice([0,1,len(text.encode())-1,len(text.encode()),len(text.encode())+256,4096]);req={'source':text,'body':'x == 1\n','full':True,'normalize':index%2==0,'max_bytes':limit};p=subprocess.run([str(a.editor)],input=json.dumps(req).encode(),capture_output=True,timeout=30);d=json.loads(p.stdout);assert d['schema_version']==1 and d['exit_code']==p.returncode and d['model_proved'] is False and d['exhaustive'] is False and d['artifact_admitted'] is False and d['accepted']==(p.returncode==0)
 if d['accepted']:
  plan=d['plan'];assert plan['algorithm_checksum_verified'] is True and len(plan['module_source'].encode())<=limit
  start,end=plan['span']['start'],plan['span']['end'];assert 0<=start<=end<=len(text.encode());rebuilt=(text.encode()[:start]+plan['replacement'].encode()+text.encode()[end:]).decode()
  if plan['newlines_normalized']:rebuilt='\n'.join(rebuilt.replace('\r\n','\n').replace('\r','\n').splitlines())+'\n'
  assert rebuilt==plan['module_source'];outcome='bounded-replacement'
 else:
  span=d['diagnostic']['span'];assert 0<=span['start']<=span['end']<=len(text.encode());text.encode()[:span['start']].decode();text.encode()[:span['end']].decode();assert d['diagnostic']['code']!='Internal' and d['result']=='refused';outcome='named-failure'
 rows.append({'index':index,'request_sha256':hashlib.sha256(json.dumps(req).encode()).hexdigest(),'outcome':outcome})
report={'schema_version':1,'controls':len(rows),'family_certified':False,'seed':17247,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'editor_sha256':hashlib.sha256(a.editor.read_bytes()).hexdigest(),'rows':rows};(BASE/'bounds-controls.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed 512 full-editor byte/span controls')
