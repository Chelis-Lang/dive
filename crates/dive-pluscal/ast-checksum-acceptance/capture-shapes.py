#!/usr/bin/env python3
"""Add a separate immutable whitespace/token/escaped-string boundary selection."""
import argparse,base64,gzip,hashlib,json,re,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main(a):
 assert sha(a.jar)==PIN;out=BASE/'control-shapes';out.mkdir();rows=[]
 forms=[
 ('if', 'L: if x=0 then x:=1; else skip; end if;', 'L: if (x=0) { x:=1; } else { skip; }'),
 ('if-no-else','L: if x=0 then x:=1; end if;','L: if (x=0) { x:=1; }'),
 ('elsif','L: if x=0 then x:=1; elsif x=1 then x:=2; else skip; end if;','L: if (x=0) { x:=1; } else { if (x=1) { x:=2; } else {skip;} }'),
 ('either','L: either x:=1; or x:=2; end either;','L: either {x:=1;} or {x:=2;}'),
 ('either-three','L: either x:=1; or x:=2; or skip; end either;','L: either {x:=1;} or {x:=2;} or {skip;}'),
 ('with','L: with a=1 do x:=a; end with;','L: with (a=1) {x:=a;}'),
 ('with-member','L: with a \\in {0,1} do x:=a; end with;','L: with (a \\in {0,1}) {x:=a;}'),
 ('with-two','L: with a=1,b=2 do x:=a+b; end with;','L: with (a=1,b=2) {x:=a+b;}'),
 ('while','L: while x<2 do x:=x+1; end while;','L: while (x<2) {x:=x+1;}'),
 ('while-mixed','L: while x<2 do x:=x+1; B: skip; end while;','L: while (x<2) {x:=x+1; B:skip;}'),
 ('while-true','L: while TRUE do x:=x+1; end while;','L: while (TRUE) {x:=x+1;}'),
 ('nested-if','L: if x=0 then if x=1 then x:=2; else x:=1; end if; else skip; end if;','L: if(x=0){if(x=1){x:=2;}else{x:=1;}}else{skip;}'),
 ('nested-with','L: with a=1 do with b=2 do x:=a+b; end with; end with;','L: with(a=1){with(b=2){x:=a+b;}}'),
 ('inner-label','L: if x=0 then B:x:=1; else skip; end if;','L: if(x=0){B:x:=1;}else{skip;}'),
 ('goto-done','L: x:=1; goto "Done";','L: x:=1; goto "Done";'),
 ]
 for syntax in ['C','P']:
  for fair in [False,True]:
   for kind,pbody,cbody in forms:
    ident=f'{syntax}_{"fair_" if fair else ""}shape_{kind}';stmt=cbody if syntax=='C' else pbody;alg=f'{{variables x=0; {{ {stmt} }} }}' if syntax=='C' else f'variables x=0; begin {stmt} end algorithm;';source=f'---- MODULE {ident.replace("-","_")} ----\nEXTENDS Integers, TLC, Sequences\n(* --{"fair algorithm" if fair else "algorithm"} A {alg} *)\n====\n';path=out/(ident+'.tla');path.write_text(source)
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
