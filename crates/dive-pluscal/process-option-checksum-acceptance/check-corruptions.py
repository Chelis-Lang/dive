#!/usr/bin/env python3
"""Configured CRC corruption controls preserve the fair-only weakness."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
p=argparse.ArgumentParser()
for n in ['java','jar','validator','native']:p.add_argument('--'+n,type=Path,required=True)
a=p.parse_args();out=BASE/'corruptions';out.mkdir();rows=[];manifest=json.loads((BASE/'primary/manifest.json').read_text())
for ident in ['P_plain_weak','C_plain_strong','C_missing_label-root','C_fair_modifiers_none']:
 row=next(r for r in manifest['rows'] if r['id']==ident);doc=json.loads(gzip.decompress((BASE/'full-controls'/(ident+'.tlc.stdout.gz')).read_bytes()));assert doc['accepted'] is True;original=doc['replacement']['module_source'];module=re.search(r'MODULE (\w+)',original).group(1)
 for kind in ['header','body','source']:
  mutated=original.replace('chksum(pcal) = "'+row['primary_crc']+'"','chksum(pcal) = "00000000"') if kind=='header' else original.replace('\\* END TRANSLATION','\\* corrupted body\n\\* END TRANSLATION') if kind=='body' else original.replace('g=0','g=9',1)
  assert mutated!=original
  with tempfile.TemporaryDirectory(prefix='dive-option-corruption-') as tmp:
   file=Path(tmp)/(module+'.tla');file.write_bytes(mutated.encode());q=subprocess.run([str(a.java),'-cp',str(a.validator)+':'+str(a.jar),'pcal.OptionValidatorProbe','-nocfg',*row['flags'],str(file)],capture_output=True,timeout=30);assert q.returncode==0;expected=b'NO_DIVERGENCE' if kind=='source' and '_fair_' in ident else b'TLA_DIVERGENCE_EXISTS' if kind=='body' else b'PCAL_DIVERGENCE_EXISTS';assert expected in q.stdout,(ident,kind,q.stdout)
   for stream in ['stdout','stderr']:(out/(ident+'.'+kind+'.validator.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
   r=subprocess.run([str(a.native),'translate',str(file),'--rewrite','--compat=tlc','--output','json',*row['flags']],capture_output=True,timeout=30);native=json.loads(r.stdout);assert native['model_proved'] is False and native['exhaustive'] is False and file.read_bytes()==mutated.encode()
   if expected==b'NO_DIVERGENCE':assert r.returncode==0 and native['accepted'] is True
   else:assert r.returncode==3 and native['accepted'] is False and 'replacement' not in native
   for stream in ['stdout','stderr']:(out/(ident+'.'+kind+'.native.'+stream+'.gz')).write_bytes(gzip.compress(getattr(r,stream),mtime=0))
   rows.append({'id':ident,'control':kind,'expected_validator':expected.decode(),'validator_exit':q.returncode,'validator_stdout_sha256':hashlib.sha256(q.stdout).hexdigest(),'native_result':native['result'],'native_exit':r.returncode,'mutated_source_sha256':hashlib.sha256(mutated.encode()).hexdigest()})
report={'schema_version':1,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'oracle_sha256':hashlib.sha256(a.jar.read_bytes()).hexdigest(),'native_sha256':hashlib.sha256(a.native.read_bytes()).hexdigest(),'family_certified':False,'controls':len(rows),'fair_header_crc_only_weakness_preserved':True,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed',len(rows),'configured corruption controls')
