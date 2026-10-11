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
 assert type(doc.get('schema_version')) is int and type(doc.get('exit_code')) is int
 assert doc.get('schema_version')==1 and doc.get('command')=='translate-rewrite' and doc.get('exit_code')==exit and doc.get('model_proved') is False and doc.get('exhaustive') is False and isinstance(doc.get('accepted'),bool)
 assert doc['editing']['checksums']=='Full'
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
 for name in ['java','jar','native','editor','validator','cst']:setattr(a,name,getattr(a,name).resolve())
 assert sha(a.jar.read_bytes())==PIN;out=BASE/'full-controls';out.mkdir(exist_ok=True);rows=[];mutations=[];primary_accept=0;native_accept=0
 selected=[]
 for folder in ['primary','corrected-c','corrected-modifiers']:
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
    cmd=[str(a.native),'translate',file.name,'--rewrite','--checksums','full','--output','json']+(['--compat=tlc'] if profile=='tlc' else [])
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
     if profile=='tlc' and ident in ['P_arithmetic','P_fair_arithmetic','C_record-field','P_changed-read']:
      (out/(ident+'.accepted.json')).write_text(json.dumps(doc,indent=2)+'\n')
      for kind in ['header-algorithm','body','source']:
       changed=native.replace(f'chksum(pcal) = "{row["primary_crc"]}"','chksum(pcal) = "00000000"') if kind=='header-algorithm' else native.replace('variables g=0','variables g=9',1) if kind=='source' else native.replace('\\* END TRANSLATION','\\* mutation\n\\* END TRANSLATION')
       if kind=='source' and changed==native:changed=native.replace('g=0','g=9',1)
       assert changed!=native,(ident,kind);file.write_text(changed);q=subprocess.run([str(a.java),'-cp',str(a.validator)+':'+str(a.jar),'pcal.ChecksumProbe','validate',str(file)],cwd=work,capture_output=True,timeout=30)
       expected='NO_DIVERGENCE' if kind=='source' and '_fair_' in ident else 'PCAL_DIVERGENCE_EXISTS' if kind in ['header-algorithm','source'] else 'TLA_DIVERGENCE_EXISTS';assert q.returncode==0 and expected.encode() in q.stdout,(ident,kind,q.stdout,q.stderr);mutations.append({'id':ident,'control':kind,'expected':expected,'capture':save(out,ident+'.'+kind,q)});file.write_text(source)
    else:
     assert not (p.returncode==1 and primary is not None and result['primary_sany']['exit_code']==0),(ident,'reference-valid input falsely rejected')
     result[profile]={'outcome':'primary-invalid-rejected' if primary is not None and result['primary_sany']['exit_code']!=0 and p.returncode==1 else outcome}
   if primary is not None and result['primary_sany']['exit_code']==0:
    p=subprocess.run([str(a.editor)],input=json.dumps({'source':source,'body':body(primary),'normalize':True,'replace_modified':True,'full':True}).encode(),capture_output=True,timeout=30);editor=json.loads(p.stdout);save(out,ident+'.editor',p);assert editor['model_proved'] is False and editor['artifact_admitted'] is False
    assert p.returncode==(0 if editor['accepted'] else 3)
    if editor['accepted']:
     plan=editor['plan'];assert plan['algorithm_checksum_verified'] is True and plan['algorithm_checksum']['checksum']==row['primary_crc'];assert plan['module_source']==primary;result['editor']={'outcome':'exact-primary-editor-bytes','artifact_admitted':False}
    else:assert editor['result']=='refused';result['editor']={'outcome':'named-refusal'}
   else:result['editor']={'outcome':'uncredited-primary-rejection-or-invalid'}
  rows.append(result);print('Accounted',len(rows),ident,flush=True)
 summary={name:dict(collections.Counter(r[name]['outcome'] for r in rows)) for name in ['dive','tlc','editor']};report={'schema_version':1,'scope':'Finite-process declared-fairness Full CRC derivation and artifact admission; unproved name/option/call/layout shapes remain refused. No full PlusCal, pretty-byte, integrity or model certification.','family_certified':False,'selected':len(rows),'primary_sany_accepted':primary_accept,'native_sany_and_validator_checks':native_accept,'corruption_controls':mutations,'script_sha256':sha(Path(__file__).read_bytes()),'native_sha256':sha(a.native.read_bytes()),'editor_sha256':sha(a.editor.read_bytes()),'cst_sha256':sha(a.cst.read_bytes()),'jar_sha256':PIN,'summary':summary,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(summary)
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for name in ['java','jar','native','editor','validator','cst']:p.add_argument('--'+name,type=Path,required=True)
 run(p.parse_args())
