#!/usr/bin/env python3
"""Add a separate immutable whitespace/token/escaped-string boundary selection."""
import argparse,base64,gzip,hashlib,json,re,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main(a):
 assert sha(a.jar)==PIN;out=BASE/'expression-controls';out.mkdir();rows=[]
 forms=[('less-equal', 'L: x:= IF 0 ≤ 1 THEN 1 ELSE 0;', 'L: x:= IF 0 ≤ 1 THEN 1 ELSE 0;'), ('not-equal', 'L: x:= IF 0 ≠ 1 THEN 1 ELSE 0;', 'L: x:= IF 0 ≠ 1 THEN 1 ELSE 0;'), ('member', 'L: x:= IF 0 ∈ {0} THEN 1 ELSE 0;', 'L: x:= IF 0 ∈ {0} THEN 1 ELSE 0;'), ('conjunction', 'L: x:= IF TRUE ∧ FALSE THEN 1 ELSE 0;', 'L: x:= IF TRUE ∧ FALSE THEN 1 ELSE 0;'), ('choose', 'L: x:= CHOOSE z ∈ {1}: z=1;', 'L: x:= CHOOSE z ∈ {1}: z=1;'), ('let', 'L: x:= LET z == 1 IN z;', 'L: x:= LET z == 1 IN z;'), ('record', 'L: x:= [a |-> 1].a;', 'L: x:= [a |-> 1].a;'), ('function', 'L: x:= [z \\in {1} |-> z][1];', 'L: x:= [z \\in {1} |-> z][1];'), ('case', 'L: x:= CASE TRUE -> 1 [] OTHER -> 0;', 'L: x:= CASE TRUE -> 1 [] OTHER -> 0;'), ('escaped', 'L: x:= "a\\t\\n\\"b";', 'L: x:= "a\\t\\n\\"b";'), ('comment', 'L: x:= 1 (* inner *) + 2;', 'L: x:= 1 (* inner *) + 2;'), ('proof-next', 'L: x:= (1 = <<1>>);', 'L: x:= (1 = <<1>>);')]
 for syntax in ['C','P']:
  for fair in [False]:
   for kind,pbody,cbody in forms:
    ident=f'{syntax}_{"fair_" if fair else ""}expression_{kind}';stmt=cbody if syntax=='C' else pbody;alg=f'{{variables x=0; {{ {stmt} }} }}' if syntax=='C' else f'variables x=0; begin {stmt} end algorithm;';source=f'---- MODULE {ident.replace("-","_")} ----\nEXTENDS Integers, TLC, Sequences\n(* --{"fair algorithm" if fair else "algorithm"} A {alg} *)\n====\n';path=out/(ident+'.tla');path.write_text(source)
    p=subprocess.run([str(a.java),'-cp',str(a.jar),'pcal.trans','-nocfg',str(path)],capture_output=True)
    for stream in ['stdout','stderr']:(out/(ident+'.primary.'+stream+'.gz')).write_bytes(gzip.compress(getattr(p,stream),mtime=0))
    raw=path.read_bytes();accepted=b'BEGIN TRANSLATION' in raw;path.write_text(source)
    if accepted:(out/(ident+'.translated.tla')).write_bytes(raw)
    q=subprocess.run([str(a.java),'-cp',str(a.adapter)+':'+str(a.jar),'pcal.AstChecksumProbe',str(path)],capture_output=True)
    for stream in ['stdout','stderr']:(out/(ident+'.adapter.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
    stages={}
    for line in q.stdout.decode().splitlines():
     if line.startswith('STAGE\t'):
      _,phase,crc,encoded,tree=line.split('\t');ast=base64.b64decode(encoded);(out/(ident+'.'+phase+'.ast')).write_bytes(ast);(out/(ident+'.'+phase+'.json')).write_bytes(base64.b64decode(tree));assert format(zlib.crc32(b'fair' if fair else ast),'x')==crc;stages[phase]={'crc':crc,'ast_sha256':hashlib.sha256(ast).hexdigest()}
    crc=re.search(rb'pcal\)\s*=\s*"([0-9a-f]+)"',raw)
    if accepted:assert 'translated' in stages and crc and stages['translated']['crc']==crc[1].decode(),ident
    rows.append({'id':ident,'source_sha256':sha(path),'primary_exit':p.returncode,'primary_translated':accepted,'primary_crc':crc[1].decode() if crc else None,'stages':stages});print(ident,accepted,flush=True)
 manifest={'schema_version':1,'jar_sha256':PIN,'adapter_sha256':sha(a.adapter/'pcal/AstChecksumProbe.class'),'source_script_sha256':sha(Path(__file__)),'selected':len(rows),'rows':rows,'files':{p.name:sha(p) for p in out.iterdir() if p.is_file()}};(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for key in ['java','jar','adapter']:p.add_argument('--'+key,type=Path,required=True)
 main(p.parse_args())
