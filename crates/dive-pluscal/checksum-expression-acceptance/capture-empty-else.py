#!/usr/bin/env python3
"""Capture complete nesting/layout selection before native checksum changes."""
import argparse,base64,gzip,hashlib,json,re,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def forms():
 return [
  ('empty-else','if x=0 then x:=1; else end if;','if(x=0){x:=1;}else{};', 'x=0,y=0'),
  ('no-else','if x=0 then x:=1; end if;','if(x=0){x:=1;};', 'x=0,y=0'),
  ('skip-else','if x=0 then x:=1; else skip; end if;','if(x=0){x:=1;}else{skip;};', 'x=0,y=0'),
  ('elsif-empty','if x=0 then x:=1; elsif x=1 then x:=2; else end if;','if(x=0){x:=1;}else{if(x=1){x:=2;}else{}};', 'x=0,y=0'),
  ('elsif-skip','if x=0 then x:=1; elsif x=1 then x:=2; else skip; end if;','if(x=0){x:=1;}else{if(x=1){x:=2;}else{skip;}};', 'x=0,y=0'),
  ('empty-then','if x=0 then else x:=1; end if;','if(x=0){}else{x:=1;};', 'x=0,y=0'),
  ('both-empty','if x=0 then else end if;','if(x=0){}else{};', 'x=0,y=0'),
  ('empty-either','either or x:=1; end either;','either{}or{x:=1;};', 'x=0,y=0'),
 ]
def main(a):
 for name in ['java','jar','adapter']:setattr(a,name,getattr(a,name).resolve())
 assert sha(a.jar)==PIN
 out=BASE/'empty-else-controls';out.mkdir();rows=[]
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
