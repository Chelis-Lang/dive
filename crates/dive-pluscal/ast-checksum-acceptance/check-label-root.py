#!/usr/bin/env python3
"""Separate clean labelRoot/name-context controls; never full-family certification."""
import argparse,base64,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
B=Path(__file__).resolve().parent
p=argparse.ArgumentParser(description=__doc__)
for key in ['java','jar','native']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();a.java=a.java.resolve();a.jar=a.jar.resolve();a.native=a.native.resolve();out=B/'label-root-controls';out.mkdir(exist_ok=True);rows=[]
def save(name,process):
 for stream in ['stdout','stderr']:(out/(name+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(process,stream),mtime=0))
for syntax in ['C','P']:
 for index,root in enumerate(['Root_','1','x','L','defaultInitValue','Done']):
  ident=f'{syntax}_root_{index}';source=f'---- MODULE {ident} ----\nEXTENDS Integers\n(* --algorithm A '+('{variables x1=0; {skip;}}' if syntax=='C' else 'variables x1=0; begin skip; end algorithm;')+' *)\n====\n';(out/(ident+'.source.tla')).write_text(source)
  with tempfile.TemporaryDirectory(prefix='dive-label-root-') as tmp:
   file=Path(tmp)/(ident+'.tla');file.write_text(source);q=subprocess.run([str(a.java),'-cp',str(a.jar),'pcal.trans','-nocfg','-labelRoot',root,str(file)],capture_output=True,timeout=30);save(ident+'.primary',q);primary=file.read_text();translated='BEGIN TRANSLATION' in primary;crc=re.search(r'chksum\(pcal\) = "([0-9a-f]+)"',primary)
   if translated:(out/(ident+'.primary.tla')).write_text(primary)
   file.write_text(source);q=subprocess.run([str(a.java),'-cp',str(B/'adapter')+':'+str(a.jar),'pcal.RootChecksumProbe',root,str(file)],capture_output=True,timeout=30);save(ident+'.adapter',q);stage=[line.split('\t') for line in q.stdout.decode().splitlines() if line.startswith('STAGE\ttranslated\t')]
   p=subprocess.run([str(a.native),'translate',str(file),'--rewrite','--label-root',root,'--compat=tlc','--output','json'],capture_output=True,timeout=30);save(ident+'.native',p);d=json.loads(p.stdout);assert file.read_text()==source and d['model_proved'] is False and d['exhaustive'] is False
   if d['accepted']:
    assert p.returncode==0 and translated and crc and len(stage)==1;ast=base64.b64decode(stage[0][3]).decode();assert d['replacement']['algorithm_checksum']['serialized_ast']==ast and d['replacement']['algorithm_checksum']['checksum']==crc[1];outcome='exact-derived-option-ast'
   else:assert p.returncode in [1,3] and 'replacement' not in d;outcome='named-refusal' if p.returncode==3 else 'rejected'
   rows.append({'id':ident,'label_root':root,'primary_translated':translated,'native_outcome':outcome,'source_sha256':hashlib.sha256(source.encode()).hexdigest()})
report={'schema_version':1,'selected':len(rows),'family_certified':False,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'jar_sha256':hashlib.sha256(a.jar.read_bytes()).hexdigest(),'native_sha256':hashlib.sha256(a.native.read_bytes()).hexdigest(),'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Accounted twelve label-root controls')
