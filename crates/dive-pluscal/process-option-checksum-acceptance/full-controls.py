#!/usr/bin/env python3
"""Option-bound public Full admission, whole CST, SANY and Validator controls."""
import argparse,collections,gzip,hashlib,json,re,subprocess,tempfile,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
import importlib.util
spec=importlib.util.spec_from_file_location('option_compare',BASE/'compare.py');comparison=importlib.util.module_from_spec(spec);spec.loader.exec_module(comparison);options=comparison.options
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(data):return hashlib.sha256(data).hexdigest()
def body(text):
 lines=text.splitlines(True);start=next(i for i,l in enumerate(lines) if 'BEGIN TRANSLATION' in l);end=next(i for i,l in enumerate(lines[start+1:],start+1) if 'END TRANSLATION' in l);return ''.join(lines[start+1:end])
def save(out,name,p):
 for stream in ['stdout','stderr']:(out/(name+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(p,stream),mtime=0))
 return {'exit_code':p.returncode,'stdout_sha256':sha(p.stdout),'stderr_sha256':sha(p.stderr)}
def integrity(doc,exit,source,row,directory,profile=None):
 assert type(doc.get('schema_version')) is int and type(doc.get('exit_code')) is int
 assert doc.get('schema_version')==1 and doc.get('command')=='translate-rewrite' and doc.get('exit_code')==exit and doc.get('model_proved') is False and doc.get('exhaustive') is False and isinstance(doc.get('accepted'),bool)
 assert doc['editing']['checksums']=='Full'
 assert json.dumps(doc['translation_options'],sort_keys=True)==json.dumps(options(row['flags']),sort_keys=True)
 if profile is not None:assert doc['profile']==doc['requested_profile']==profile and doc['overrides']==[]
 assert doc['result']=={0:'replacement',1:'rejected',3:'unsupported',4:'internal-error'}[exit] and doc['accepted']==(exit==0)
 if exit:
  assert exit in [1,3] and 'replacement' not in doc
  cause=doc.get('generation',doc)
  if cause is not doc:
   assert cause['schema_version']==1 and cause['command']=='translate' and cause['exit_code']==exit and cause['accepted'] is False and cause['model_proved'] is False and cause['exhaustive'] is False and cause['result']==('unsupported' if exit==3 else 'rejected')
   if profile is not None:assert cause['profile']==profile
  if exit==3:
   diagnostic=cause['diagnostic'];assert diagnostic['code'] in ['Unsupported','Resource','DIVE-P0090','DIVE-P0040'] and bool(diagnostic['message'])
   if 'code' in cause:assert cause['code'] in ['DIVE-P0090','DIVE-P0040']
  elif 'analysis' in cause:
   assert cause['analysis']['accepted'] is False and any(d['severity']=='error' and bool(d['code']) for d in cause['analysis']['diagnostics'])
  else:
   assert cause['diagnostic']['code'] in ['Lexical','Syntax','Expression','Label','DIVE-P0001','DIVE-P0002','DIVE-P0003','DIVE-P0004'] and bool(cause['diagnostic']['message'])
  return 'named-refusal' if exit==3 else 'rejected'
 assert doc['stage']=='replacement-analysis'
 assert row['primary_translated'] and doc['algorithm_checksum_verified'] is True and doc['body_checksum_verified'] is True and doc['exact_primary_generated_bytes'] is False and doc['analysis']['accepted'] is True
 plan=doc['replacement'];assert plan['algorithm_checksum_verified'] is True;ast=plan['algorithm_checksum'];assert ast['serialized_ast']==(directory/(row['id']+'.translated.ast')).read_text() and ast['checksum']==row['primary_crc'] and ast['fair_header_crc_only']==('_fair_' in row['id'])
 assert format(zlib.crc32(('fair' if ast['fair_header_crc_only'] else ast['serialized_ast']).encode()),'x')==ast['checksum']
 encoded=source.encode();start,end=plan['span']['start'],plan['span']['end'];assert type(start) is int and type(end) is int and 0<=start<=end<=len(encoded);rebuilt=(encoded[:start]+plan['replacement'].encode()+encoded[end:]).decode()
 if plan['newlines_normalized']:rebuilt='\n'.join(rebuilt.replace('\r\n','\n').replace('\r','\n').splitlines())+'\n'
 assert rebuilt==plan['module_source'];observed=body(plan['module_source']);newline='\n' if plan['newlines_normalized'] else re.search(r'\r\n|\r|\n',source).group(0);assert observed==''.join(line+newline for line in doc['artifact']['tla'].splitlines());crc=format(zlib.crc32(''.join(observed.splitlines()).encode()),'x');assert crc==plan['translation_checksum'];assert f'chksum(pcal) = "{ast["checksum"]}"' in plan['module_source'] and f'chksum(tla) = "{crc}"' in plan['module_source'];return 'full-derivation-and-admission'
def run(a):
 for name in ['java','jar','native','validator','cst']:setattr(a,name,getattr(a,name).resolve())
 assert sha(a.jar.read_bytes())==PIN;out=BASE/'full-controls';out.mkdir();rows=[];selected=[];manifests={}
 for folder in ['primary','context','label-options']:
  directory=BASE/folder;m=json.loads((directory/'manifest.json').read_text());manifests[folder]=sha((directory/'manifest.json').read_bytes());selected.extend((directory,row) for row in m['rows'])
 for directory,row in selected:
  ident=row['id'];source=(directory/(ident+'.tla')).read_bytes().decode();module=re.search(r'MODULE (\w+)',source).group(1);flags=row['flags'];result={'id':ident,'source_sha256':sha(source.encode()),'flags':flags}
  with tempfile.TemporaryDirectory(prefix='dive-process-option-full-') as tmp:
   work=Path(tmp);file=work/(module+'.tla');file.write_bytes(source.encode())
   invalid_options=('-wf' in flags and '-sf' in flags) or any(flag in flags and not re.fullmatch('[A-Za-z][A-Za-z0-9_]*',flags[flags.index(flag)+1]) for flag in ['-labelRoot'])
   if row['primary_translated']:
    primary=(directory/(ident+'.translated.tla')).read_bytes().decode();file.write_bytes(primary.encode());q=subprocess.run([str(a.java),'-cp',str(a.jar),'tla2sany.SANY','-error-codes',file.name],cwd=work,capture_output=True,timeout=30);result['primary_sany']=save(out,ident+'.primary-sany',q);file.write_bytes(source.encode())
   for profile in ['dive','tlc']:
    cmd=[str(a.native),'translate',file.name,'--rewrite','--checksums','full','--output','json',*flags]+(['--compat=tlc'] if profile=='tlc' else []);p=subprocess.run(cmd,cwd=work,capture_output=True,timeout=30);saved=save(out,ident+'.'+profile,p);assert file.read_bytes()==source.encode()
    if '-wf' in flags and '-sf' in flags:
     assert p.returncode==2 and not p.stdout and b'select only one' in p.stderr
     result[profile]={'outcome':'named-cli-option-rejection','capture':saved};continue
    doc=json.loads(p.stdout);outcome=integrity(doc,p.returncode,source,row,directory,profile)
    assert doc['translation_options']==options(flags),(ident,doc['translation_options'],options(flags))
    if p.returncode==0:
     assert result['primary_sany']['exit_code']==0,(ident,'invalid primary admitted')
     native=doc['replacement']['module_source'];file.write_bytes(native.encode());q=subprocess.run([str(a.java),'-cp',str(a.jar),'tla2sany.SANY','-error-codes',file.name],cwd=work,capture_output=True,timeout=30);assert q.returncode==0,(ident,q.stdout);sany=save(out,ident+'.'+profile+'.native-sany',q)
     structures=[]
     for artifact in [primary,native]:
      q=subprocess.run([str(a.cst),'--cst'],input=artifact.encode(),capture_output=True,timeout=30);assert q.returncode==0;tree=json.loads(q.stdout);assert tree['accepted'] is True;structures.append(tree['structure'])
     assert structures[0]==structures[1],(ident,'whole-module CST mismatch')
     q=subprocess.run([str(a.java),'-cp',str(a.validator)+':'+str(a.jar),'pcal.OptionValidatorProbe','-nocfg',*flags,str(file)],cwd=work,capture_output=True,timeout=30);assert q.returncode==0 and b'NO_DIVERGENCE' in q.stdout,(ident,q.stdout,q.stderr);validation=save(out,ident+'.'+profile+'.validator',q);file.write_bytes(source.encode())
     result[profile]={'outcome':outcome,'capture':saved,'sany':sany,'validator':validation,'module_sha256':sha(native.encode()),'operator_structure_equal':True,'operator_structure_sha256':sha(json.dumps(structures[0],ensure_ascii=False).encode())}
     if profile=='tlc' and ident in ['P_plain_weak','P_fair_modifiers_none','C_missing_label-root','P_owners_none']:(out/(ident+'.accepted.json')).write_text(json.dumps(doc,indent=2)+'\n')
    else:result[profile]={'outcome':outcome,'capture':saved}
  rows.append(result);print(ident,{p:result[p]['outcome'] for p in ['dive','tlc']},flush=True)
 summary={p:dict(collections.Counter(r[p]['outcome'] for r in rows)) for p in ['dive','tlc']}
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'jar_sha256':PIN,'native_sha256':sha(a.native.read_bytes()),'cst_sha256':sha(a.cst.read_bytes()),'validator_sha256':sha((a.validator/'pcal/OptionValidatorProbe.class').read_bytes()),'manifests':manifests,'selected':len(rows),'model_proved':False,'family_certified':False,'summary':summary,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(summary)
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for n in ['java','jar','native','validator','cst']:p.add_argument('--'+n,type=Path,required=True)
 run(p.parse_args())
