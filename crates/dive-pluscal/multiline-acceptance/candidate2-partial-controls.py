#!/usr/bin/env python3
"""Independent Full-report integrity, primary admission and validator controls."""
import argparse,collections,gzip,hashlib,json,re,subprocess,tempfile,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(data):return hashlib.sha256(data).hexdigest()
def body(text):
 lines=text.splitlines(True);start=next(i for i,l in enumerate(lines) if 'BEGIN TRANSLATION' in l);end=next(i for i,l in enumerate(lines[start+1:],start+1) if 'END TRANSLATION' in l);return ''.join(lines[start+1:end])
def save(out,name,p):
 for stream in ['stdout','stderr']:(out/(name+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(p,stream),mtime=0))
 return {'exit_code':p.returncode,'stdout_sha256':sha(p.stdout),'stderr_sha256':sha(p.stderr)}
def integrity(doc,exit,source,row,directory,profile=None):
 assert doc.get('schema_version')==1 and doc.get('command')=='translate-rewrite' and doc.get('exit_code')==exit and doc.get('model_proved') is False and doc.get('exhaustive') is False and isinstance(doc.get('accepted'),bool)
 assert doc['editing']['checksums']=='TranslationOnly'
 if profile is not None:assert doc['requested_profile']==profile and doc['profile']==('tlc+overrides' if profile=='tlc' else 'dive') and doc['overrides']==['translation-only-checksum']
 assert doc['result']=={0:'replacement',1:'rejected',3:'unsupported',4:'internal-error'}[exit] and doc['accepted']==(exit==0)
 if exit:
  assert exit in [1,3] and 'replacement' not in doc
  cause=doc.get('generation',doc);assert cause['accepted'] is False and cause['model_proved'] is False and cause['exhaustive'] is False
  if cause is not doc:assert cause['schema_version']==1 and cause['command']=='translate' and cause['exit_code']==exit and cause['result']==('unsupported' if exit==3 else 'rejected')
  if exit==3:assert cause['diagnostic']['code'] in ['Unsupported','Resource','DIVE-P0090','DIVE-P0040']
  if exit==3:assert bool(cause['diagnostic']['message'])
  return 'named-refusal' if exit==3 else 'rejected'
 assert doc['exact_primary_generated_bytes'] is False
 assert doc['stage']=='replacement-analysis' and row['primary_translated'] and doc['algorithm_checksum_verified'] is False and doc['body_checksum_verified'] is True and doc['analysis']['accepted'] is True
 plan=doc['replacement'];assert plan['algorithm_checksum_verified'] is False and plan.get('algorithm_checksum') is None
 encoded=source.encode();start,end=plan['span']['start'],plan['span']['end'];assert type(start) is int and type(end) is int and 0<=start<=end<=len(encoded);rebuilt=(encoded[:start]+plan['replacement'].encode()+encoded[end:]).decode()
 if plan['newlines_normalized']:rebuilt='\n'.join(rebuilt.replace('\r\n','\n').replace('\r','\n').splitlines())+'\n'
 assert rebuilt==plan['module_source'];observed=body(plan['module_source']);newline='\n' if plan['newlines_normalized'] else re.search(r'\r\n|\r|\n',source).group(0);assert observed==''.join(line+newline for line in doc['artifact']['tla'].splitlines());crc=format(zlib.crc32(''.join(observed.splitlines()).encode()),'x');assert crc==plan['translation_checksum'];assert 'chksum(pcal) \\in STRING' in plan['module_source'];return 'body-derivation-and-admission'
def run(a):
 for name in ['java','jar','native','editor','validator','cst']:setattr(a,name,getattr(a,name).resolve())
 assert sha(a.jar.read_bytes())==PIN;out=BASE/'partial-controls';out.mkdir(exist_ok=True);rows=[];mutations=[];primary_accept=0;native_accept=0
 selected=[]
 for folder in ['primary','layout-controls','supplemental','bool-controls']:
  directory=BASE/folder;m=json.loads((directory/'manifest.json').read_text());selected.extend((directory,row) for row in m['rows'])
 for directory,row in selected:
  original=row['id'];ident=original if directory.name=='primary' else directory.name+'__'+original;source=(directory/(original+'.tla')).read_bytes().decode();module=re.search(r'MODULE (\w+)',source).group(1)
  result={'id':ident,'primary_translated':row['primary_translated'],'source_sha256':sha(source.encode())}
  with tempfile.TemporaryDirectory(prefix='dive-full-crc-') as tmp:
   work=Path(tmp);file=work/(module+'.tla')
   if row['primary_translated']:
    primary=(directory/(original+'.translated.tla')).read_bytes().decode();file.write_text(primary)
    p=subprocess.run([str(a.java),'-cp',str(a.jar),'tla2sany.SANY','-error-codes',file.name],cwd=work,capture_output=True,timeout=30);result['primary_sany']=save(out,ident+'.primary-sany',p);primary_accept+=p.returncode==0
   else:primary=None
   file.write_text(source)
   for profile in ['dive','tlc']:
    cmd=[str(a.native),'translate',file.name,'--rewrite','--checksums','tla','--output','json']+(['--compat=tlc'] if profile=='tlc' else [])
    p=subprocess.run(cmd,cwd=work,capture_output=True,timeout=30);doc=json.loads(p.stdout);assert file.read_bytes().decode()==source;save(out,ident+'.'+profile,p)
    outcome=integrity(doc,p.returncode,source,row,directory,profile)
    if p.returncode==0:
     assert result['primary_sany']['exit_code']==0,(ident,'invalid primary admitted')
     native=doc['replacement']['module_source'];file.write_text(native)
     q=subprocess.run([str(a.java),'-cp',str(a.jar),'tla2sany.SANY','-error-codes',file.name],cwd=work,capture_output=True,timeout=30);assert q.returncode==0,(ident,q.stdout);sany=save(out,ident+'.'+profile+'.native-sany',q)
     structures=[]
     for artifact in [primary,native]:
      q=subprocess.run([str(a.cst),'--cst'],input=artifact.encode(),capture_output=True,timeout=30);assert q.returncode==0;tree=json.loads(q.stdout);assert tree['accepted'] is True;structures.append(tree['structure'])
     assert structures[0]==structures[1],(ident,'operator CST mismatch')
     q=subprocess.run([str(a.java),'-cp',str(a.validator)+':'+str(a.jar),'pcal.ChecksumProbe','validate',str(file)],cwd=work,capture_output=True,timeout=30);assert q.returncode==0 and b'NO_DIVERGENCE' in q.stdout,(ident,q.stdout,q.stderr);validation=save(out,ident+'.'+profile+'.validator',q);file.write_text(source);native_accept+=1
     result[profile]={'outcome':outcome,'sany':sany,'validator':validation,'module_sha256':sha(native.encode()),'operator_structure_equal':True,'operator_structure_sha256':sha(json.dumps(structures[0],ensure_ascii=False).encode())}
    else:
     assert not (p.returncode==1 and primary is not None and result['primary_sany']['exit_code']==0),(ident,'reference-valid input falsely rejected')
     result[profile]={'outcome':'primary-invalid-rejected' if primary is not None and result['primary_sany']['exit_code']!=0 and p.returncode==1 else outcome}
   result['editor']={'outcome':'not-selected'}
  rows.append(result);print('Accounted',len(rows),ident,flush=True)
 summary={name:dict(collections.Counter(r[name]['outcome'] for r in rows)) for name in ['dive','tlc','editor']};report={'schema_version':1,'scope':'Non-junction multiline generation, partial body-only CRC derivation and artifact admission; junction/macro/process/procedure layouts remain refused. No full PlusCal, pretty-byte or model certification.','family_certified':False,'selected':len(rows),'primary_sany_accepted':primary_accept,'native_sany_and_validator_checks':native_accept,'corruption_controls':mutations,'script_sha256':sha(Path(__file__).read_bytes()),'native_sha256':sha(a.native.read_bytes()),'editor_sha256':sha(a.editor.read_bytes()),'cst_sha256':sha(a.cst.read_bytes()),'jar_sha256':PIN,'summary':summary,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(summary)
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for name in ['java','jar','native','editor','validator','cst']:p.add_argument('--'+name,type=Path,required=True)
 run(p.parse_args())
