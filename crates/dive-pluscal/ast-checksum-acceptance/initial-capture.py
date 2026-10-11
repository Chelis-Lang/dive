#!/usr/bin/env python3
"""Capture clean pinned translation and all three development AST stages once."""
import argparse,base64,gzip,hashlib,json,re,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,data):p.write_bytes(gzip.compress(data,mtime=0))
def main(a):
 assert sha(a.jar)==PIN
 out=BASE/'primary';out.mkdir();selection=[];rows=[]
 cases=[('skip','variables x=0;', 'L: skip;'),('assign','variables x=0;', 'L: x := x+1;'),('two-labels','variables x=0;', 'L: x:=1; E: skip;'),('parallel','variables x=0,y=1;','L: x:=y || y:=x;'),('goto','variables x=0;','L: x:=1; goto L;'),('await','variables x=0;','L: await x=0; x:=1;'),('assert','variables x=0;','L: assert x=0; x:=1;'),('print','variables x=0;','L: print x; x:=1;'),('member','variables x \\in {0,1};','L: x:=1;'),('default','variables x;','L: x:=1;'),('selector','variables x=<<0,1>>;','L: x[1]:=1;'),('string','variables x="a";','L: x:="b";'),('backslash','variables x={0};','L: x := x \\cup {1};'),('if','variables x=0;','L: if x=0 then x:=1; else skip; end if;'),('either','variables x=0;','L: either x:=1; or x:=2; end either;'),('with','variables x=0;','L: with a=1 do x:=a; end with;'),('while','variables x=0;','L: while x<2 do x:=x+1; end while;'),('inner-label','variables x=0;','L: while x<2 do B:x:=x+1; end while;'),('junction','variables x=0;','L: await /\\ TRUE /\\ x=0;'),('multiline','variables x=0;','L: x := (1 +\n 2);'),('implicit','variables x=0;','x:=1;'),('label-modifier','variables x=0;','L:+ x:=1;'),('undefined','variables x=0;','L: x:=q;'),('define','variables x=0; define Z==1 end define;','L:x:=Z;')]
 for syntax in ['P','C']:
  for fair in [False,True]:
   for kind,decls,body in cases:
    if syntax=='C':
     if kind in ['if','either','with','while','inner-label','define']:continue
     algorithm=f'{{ {decls} {{ {body} }} }}'
    else:algorithm=f'{decls}\nbegin {body} end algorithm;'
    ident=f'{syntax}_{"fair_" if fair else ""}{kind}';source=f'---- MODULE {ident.replace("-","_")} ----\nEXTENDS Integers, TLC, Sequences\n(* --{"fair algorithm" if fair else "algorithm"} A {algorithm} *)\n====\n';selection.append({'id':ident,'syntax':syntax,'fair':fair,'kind':kind,'source':source})
 (BASE/'selection.json').write_text(json.dumps(selection,indent=2)+'\n')
 for item in selection:
  ident=item['id'];path=out/(ident+'.tla');path.write_text(item['source']);cmd=[str(a.java),'-cp',str(a.jar),'pcal.trans','-nocfg',str(path)]
  r=subprocess.run(cmd,capture_output=True);save(out/(ident+'.primary.stdout.gz'),r.stdout);save(out/(ident+'.primary.stderr.gz'),r.stderr)
  raw=path.read_bytes(); translated=b'BEGIN TRANSLATION' in raw; (out/(ident+'.translated.tla')).write_bytes(raw) if translated else None
  # Preserve original even though clean translator overwrote the working file.
  path.write_text(item['source'])
  adapter=[str(a.java),'-cp',str(a.adapter)+':'+str(a.jar),'pcal.AstChecksumProbe',str(path)]
  q=subprocess.run(adapter,capture_output=True);save(out/(ident+'.adapter.stdout.gz'),q.stdout);save(out/(ident+'.adapter.stderr.gz'),q.stderr)
  stages={}
  for line in q.stdout.decode().splitlines():
   if line.startswith('STAGE\t'):
    _,phase,crc,encoded,tree=line.split('\t');ast=base64.b64decode(encoded);(out/(ident+'.'+phase+'.ast')).write_bytes(ast);(out/(ident+'.'+phase+'.json')).write_bytes(base64.b64decode(tree));assert format(zlib.crc32(b'fair' if item['fair'] else ast),'x')==crc;stages[phase]={'crc':crc,'ast_sha256':hashlib.sha256(ast).hexdigest()}
  primary_crc=re.search(rb'pcal\s*=\s*"([0-9a-f]+)"',raw)
  if translated and 'translated' in stages:assert primary_crc and primary_crc[1].decode()==stages['translated']['crc'],ident
  rows.append({'id':ident,'source_sha256':sha(path),'primary_exit':r.returncode,'primary_translated':translated,'primary_crc':primary_crc[1].decode() if primary_crc else None,'stages':stages,'command':cmd[1:],'adapter_command':adapter[1:]})
  print(ident,translated,list(stages),flush=True)
 files={p.name:sha(p) for p in out.iterdir() if p.is_file()}
 (out/'manifest.json').write_text(json.dumps({'schema_version':1,'jar_sha256':PIN,'java_sha256':sha(a.java),'adapter_sha256':sha(a.adapter/'pcal/AstChecksumProbe.class'),'adapter_source_sha256':sha(BASE/'AstChecksumProbe.java'),'selected':len(selection),'rows':rows,'files':files},indent=2)+'\n')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for name in ['java','jar','adapter']:p.add_argument('--'+name,type=Path,required=True)
 main(p.parse_args())
