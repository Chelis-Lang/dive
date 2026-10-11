#!/usr/bin/env python3
"""Capture complete nesting/layout selection before native checksum changes."""
import argparse,base64,gzip,hashlib,json,re,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def forms():
 rows=[]
 def add(name,p,c=None,decl='x=0,y=0'):rows.append((name,p,p if c is None else c,decl))
 add('arithmetic','x:=1 +\n  2;')
 add('blank','x:=1 +\n\n  2;')
 add('block-comment','x:=1 + (* α\n β *) 2;')
 add('line-comment','x:=1 + \\* α comment\n  2;')
 add('tuple','x:=<<1,\n  2>>;')
 add('record','x:=[a |-> 1,\n  b |-> 2];')
 add('if-value','x:=IF y=0\n  THEN 1\n  ELSE 2;')
 add('case-value','x:=CASE y=0 -> 1\n  [] OTHER -> 2;')
 add('declaration','skip;',decl='x=<<1,\n  2>>,y=0')
 add('parallel','x:=y +\n  1 || y:=x +\n  2;')
 add('changed-read','x:=1; y:=x +\n  1;')
 add('record-field','x:=1; y:=[x |-> x,\n  z |-> x].x;')
 add('selector','x[1]:=1 +\n  2;',decl='x=<<0,0>>,y=0')
 add('multiline-selector','x[1 +\n  0]:=2;',decl='x=<<0,0>>,y=0')
 add('await','await (y=0\n  = TRUE);')
 add('assert','assert y=0\n  = TRUE;')
 add('print','print <<x,\n  y>>;')
 add('if-test','if y=0\n  = TRUE then x:=1; else x:=2; end if;','if(y=0\n  = TRUE){x:=1;}else{x:=2;};')
 add('while-test','while x<2\n  = TRUE do x:=x+1; end while;','while(x<2\n  = TRUE){x:=x+1;};')
 add('with-value','with a=1 +\n  2 do x:=a; end with;','with(a=1 +\n  2){x:=a;};')
 add('junction','await /\\ TRUE\n  /\\ y=0;')
 add('junction-value','x:=IF /\\ TRUE\n  /\\ y=0 THEN 1 ELSE 2;')
 add('unicode-junction','await ∧ TRUE\n  ∧ y=0;')
 add('binder','x:=CHOOSE z \\in {1,\n  2}:TRUE;')
 add('function-binder','x:=[z \\in {1,\n  2} |-> z];')
 add('empty-then','if x=0 then else x:=1+\n  2; end if;','if(x=0){}else{x:=1+\n  2;};')
 add('empty-else','if x=0 then x:=1+\n  2; else end if;','if(x=0){x:=1+\n  2;}else{};')
 add('malformed','x:=1 +\n ;')
 add('missing-selector','x[\n]:=1;')
 add('macro','M();',decl='x=0,y=0; macro M() begin x:=1+\n 2; end macro')
 return rows
def main(a):
 for name in ['java','jar','adapter']:setattr(a,name,getattr(a,name).resolve())
 assert sha(a.jar)==PIN
 out=BASE/'primary';out.mkdir();rows=[]
 for syntax in ['C','P']:
  for fair in [False,True]:
   for kind,pbody,cbody,decl in forms():
    ident=f'{syntax}_{"fair_" if fair else ""}{kind}';module=ident.replace('-','_');body=cbody if syntax=='C' else pbody
    alg=f'{{variables {decl}; {{ L:{body} E:skip; }} }}' if syntax=='C' else f'variables {decl}; begin L:{body} E:skip; end algorithm;'
    source=f'---- MODULE {module} ----\nEXTENDS Integers, TLC, Sequences\n(* --{"fair algorithm" if fair else "algorithm"} A {alg} *)\n====\n'
    path=out/(ident+'.tla');path.write_text(source)
    p=subprocess.run([str(a.java),'-cp',str(a.jar),'pcal.trans','-nocfg',str(path)],capture_output=True,timeout=30)
    for stream in ['stdout','stderr']:(out/(ident+'.primary.'+stream+'.gz')).write_bytes(gzip.compress(getattr(p,stream),mtime=0))
    raw=path.read_bytes();accepted=b'BEGIN TRANSLATION' in raw;path.write_text(source)
    if accepted:(out/(ident+'.translated.tla')).write_bytes(raw)
    q=subprocess.run([str(a.java),'-cp',str(a.adapter)+':'+str(a.jar),'pcal.AstChecksumProbe',str(path)],capture_output=True,timeout=30)
    for stream in ['stdout','stderr']:(out/(ident+'.adapter.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
    stages={}
    for line in q.stdout.decode().splitlines():
     if line.startswith('STAGE\t'):
      _,phase,crc,encoded,tree=line.split('\t');ast=base64.b64decode(encoded);(out/(ident+'.'+phase+'.ast')).write_bytes(ast);(out/(ident+'.'+phase+'.json')).write_bytes(base64.b64decode(tree));assert format(zlib.crc32(b'fair' if fair else ast),'x')==crc;stages[phase]={'crc':crc,'ast_sha256':hashlib.sha256(ast).hexdigest()}
    crc=re.search(rb'pcal\)\s*=\s*"([0-9a-f]+)"',raw)
    if accepted:assert 'translated' in stages and crc and stages['translated']['crc']==crc[1].decode(),ident
    rows.append({'id':ident,'source_sha256':sha(path),'primary_exit':p.returncode,'primary_translated':accepted,'primary_crc':crc[1].decode() if crc else None,'stages':stages});print(ident,accepted,flush=True)
 manifest={'schema_version':1,'jar_sha256':PIN,'adapter_sha256':sha(a.adapter/'pcal/AstChecksumProbe.class'),'source_script_sha256':sha(Path(__file__)),'selected':len(rows),'rows':rows,'files':{p.name:sha(p) for p in out.iterdir() if p.is_file()}}
 (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for key in ['java','jar','adapter']:p.add_argument('--'+key,type=Path,required=True)
 main(p.parse_args())
