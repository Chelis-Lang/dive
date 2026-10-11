#!/usr/bin/env python3
"""Clean pinned option/context capture before lifting any native Full refusal."""
import argparse,base64,gzip,hashlib,json,re,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
OPTIONS=[('declared',[]),('none',['-nof']),('weak',['-wf']),('strong',['-sf']),('next',['-wfNext']),('label',['-label']),('root-only',['-labelRoot','Auto']),('label-root',['-label','-labelRoot','Auto']),('none-label',['-nof','-label']),('weak-label',['-wf','-label']),('strong-label',['-sf','-label']),('next-label',['-wfNext','-label']),('invalid-combination',['-wf','-sf'])]
FORMS=[('plain','','L:g:=1; E:skip;',None),('weak','fair ','L:g:=1; E:skip;',None),('strong','fair+ ','L:g:=1; E:skip;',None),('modifiers','fair+ ','L:-g:=1; M:+g:=2; E:skip;',None),('missing','','g:=1; g:=2;',None),('loop','','while g<2 do g:=g+1; end while;', 'while(g<2){g:=g+1;}'),('partial','','L:g:=1; g:=2;',None)]
def main(a):
 for k in ['java','jar','adapter']:setattr(a,k,getattr(a,k).resolve())
 assert sha(a.jar)==PIN
 out=BASE/'primary';out.mkdir();rows=[]
 for syntax in ['C','P']:
  for fair in [False,True]:
   for name,declared,pbody,cbody in FORMS:
    for option,flags in OPTIONS:
     ident=f'{syntax}_{"fair_" if fair else ""}{name}_{option}';module=ident.replace('-','_')
     algorithm=('variables g=0; '+declared+'process P \\in {1,2} begin '+pbody+' end process; end algorithm;') if syntax=='P' else ('{variables g=0; '+declared+'process(P \\in {1,2}){'+(cbody or pbody)+'}}')
     source=f'---- MODULE {module} ----\nEXTENDS Integers, TLC, Sequences\n(* --{"fair algorithm" if fair else "algorithm"} A {algorithm} *)\n====\n'
     path=out/(ident+'.tla');path.write_bytes(source.encode());p=subprocess.run([str(a.java),'-cp',str(a.jar),'pcal.trans','-nocfg',*flags,str(path)],capture_output=True,timeout=30)
     for stream in ['stdout','stderr']:(out/(ident+'.primary.'+stream+'.gz')).write_bytes(gzip.compress(getattr(p,stream),mtime=0))
     raw=path.read_bytes();accepted=b'BEGIN TRANSLATION' in raw;path.write_bytes(source.encode())
     if accepted:(out/(ident+'.translated.tla')).write_bytes(raw)
     q=subprocess.run([str(a.java),'-cp',str(a.adapter)+':'+str(a.jar),'pcal.OptionChecksumProbe','-nocfg',*flags,str(path)],capture_output=True,timeout=30)
     for stream in ['stdout','stderr']:(out/(ident+'.adapter.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
     stages={}
     for line in q.stdout.decode().splitlines():
      if line.startswith('STAGE\t'):
       _,phase,crc,encoded,tree=line.split('\t');ast=base64.b64decode(encoded);(out/(ident+'.'+phase+'.ast')).write_bytes(ast);(out/(ident+'.'+phase+'.json')).write_bytes(base64.b64decode(tree));assert format(zlib.crc32(b'fair' if fair else ast),'x')==crc;stages[phase]={'crc':crc,'ast_sha256':hashlib.sha256(ast).hexdigest()}
     crc=re.search(rb'pcal\)\s*=\s*"([0-9a-f]+)"',raw)
     if accepted:assert 'translated' in stages and crc and stages['translated']['crc']==crc[1].decode(),ident
     rows.append({'id':ident,'flags':flags,'source_sha256':sha(path),'primary_exit':p.returncode,'primary_translated':accepted,'primary_crc':crc[1].decode() if crc else None,'stages':stages});print(ident,accepted,flush=True)
 manifest={'schema_version':1,'jar_sha256':PIN,'adapter_sha256':sha(a.adapter/'pcal/OptionChecksumProbe.class'),'adapter_source_sha256':sha(BASE/'OptionChecksumProbe.java'),'source_script_sha256':sha(Path(__file__)),'selected':len(rows),'family_certified':False,'rows':rows,'files':{p.name:sha(p) for p in out.iterdir() if p.is_file()}}
 (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for k in ['java','jar','adapter']:p.add_argument('--'+k,type=Path,required=True)
 main(p.parse_args())
