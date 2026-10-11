#!/usr/bin/env python3
"""Capture complete nesting/layout selection before native checksum changes."""
import argparse,base64,gzip,hashlib,json,re,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def forms():
 rows=[]
 def add(name,p,c=None,decl='x=0,y=0'):rows.append((name,p,c if c is not None else p,decl))
 add('elsif-two','if x=0 then x:=1; elsif x=1 then x:=2; else skip; end if;','if(x=0){x:=1;}else{if(x=1){x:=2;}else{skip;}}')
 add('elsif-three','if x=0 then x:=1; elsif x=1 then x:=2; elsif x=2 then skip; else x:=3; end if;','if(x=0){x:=1;}else{if(x=1){x:=2;}else{if(x=2){skip;}else{x:=3;}}}')
 add('elsif-no-else','if x=0 then x:=1; elsif x=1 then x:=2; end if;','if(x=0){x:=1;}else{if(x=1){x:=2;}}')
 add('elsif-in-while','while x<2 do if x=0 then x:=1; elsif x=1 then x:=2; end if; end while;','while(x<2){if(x=0){x:=1;}else{if(x=1){x:=2;}}}')
 add('with-two','with a=1,b=2 do x:=a+b; end with;','with(a=1,b=2){x:=a+b;}')
 add('with-three','with a=1,b=2,c=3 do x:=a+b+c; end with;','with(a=1,b=2,c=3){x:=a+b+c;}')
 add('with-member','with a \\in {0,1},b \\in {1,2} do x:=a+b; end with;','with(a \\in {0,1},b \\in {1,2}){x:=a+b;}')
 add('with-dependent','with a=1,b=a+1 do x:=a+b; end with;','with(a=1,b=a+1){x:=a+b;}')
 add('with-shadow','with x=1,b=2 do y:=x+b; end with;','with(x=1,b=2){y:=x+b;}')
 add('with-nested','with a=1,b=2 do with c=3,d=4 do x:=a+b+c+d; end with; end with;','with(a=1,b=2){with(c=3,d=4){x:=a+b+c+d;}}')
 add('plus-lines','x:=1 +\n      2;')
 add('plus-blank','x:=1 +\n\n      2;')
 add('plus-comment','x:=1 + (* retained comment *)\n      2;')
 add('plus-line-comment','x:=1 + \\* retained comment\n      2;')
 add('tuple-lines','x:=<<1,\n       2>>;')
 add('record-lines','x:=[a |-> 1,\n        b |-> 2];')
 add('if-lines','x:=IF y=0\n      THEN 1\n      ELSE 2;')
 add('case-lines','x:=CASE y=0 -> 1\n        [] OTHER -> 2;')
 add('decl-lines','skip;',decl='x = <<1,\n      2>>, y=0')
 add('junction-one','await /\\ TRUE /\\ y=0;')
 add('junction-or','await \\/ FALSE \\/ y=0;')
 add('junction-lines','await /\\ TRUE\n      /\\ y=0;')
 add('junction-left','await /\\ TRUE\n /\\ y=0;')
 add('junction-right','await /\\ TRUE\n                /\\ y=0;')
 add('junction-tabs','await /\\ TRUE\n\t/\\ y=0;')
 add('junction-comment','await /\\ TRUE (* inside *)\n      /\\ y=0;')
 add('junction-nested','await /\\ (y=0 \\/ y=1)\n      /\\ TRUE;')
 add('junction-rhs','x:=IF /\\ TRUE\n         /\\ y=0 THEN 1 ELSE 2;')
 add('junction-unicode','await ∧ TRUE\n      ∧ y=0;')
 add('escaped-string','x:="left\\nright";')
 add('nested-elsif-with','with a=1,b=2 do if x=0 then x:=a; elsif x=1 then x:=b; else skip; end if; end with;','with(a=1,b=2){if(x=0){x:=a;}else{if(x=1){x:=b;}else{skip;}}}')
 add('branch-label-refusal','if x=0 then B:x:=1; else skip; end if;','if(x=0){B:x:=1;}else{skip;}')
 add('proof-token-refusal','x:=<1>2;')
 add('malformed-junction','await /\\ ;')
 add('missing-elsif-test','if x=0 then x:=1; elsif then skip; end if;','if(x=0){x:=1;}else{if(){skip;}}')
 add('with-missing-binder','with a=1, do skip; end with;','with(a=1,){skip;}')
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
