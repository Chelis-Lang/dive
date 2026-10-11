#!/usr/bin/env python3
"""Stage-separated primary-byte editor controls and native generation admission."""
import argparse,collections,gzip,hashlib,json,re,subprocess,tempfile,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
DIRS=[BASE,BASE/'partial-headers',BASE/'corrected-c',BASE/'fair-checksum']
def sha(data):return hashlib.sha256(data).hexdigest()
def execute(binary,document):
 p=subprocess.run([str(binary)],input=json.dumps(document).encode(),capture_output=True,timeout=30)
 return p,json.loads(p.stdout)
def body_of(text):
 lines=text.splitlines(keepends=True);beg=next(i for i,l in enumerate(lines) if re.search(r'\bBEGIN +TRANSLATION\b',l));end=next(i for i,l in enumerate(lines[beg+1:],beg+1) if re.search(r'\bEND +TRANSLATION\b',l));return ''.join(lines[beg+1:end])
def integrity(doc,exit,operation):
 return doc.get('result')==('replacement' if exit==0 else 'refused' if operation=='source-editor' else 'unsupported' if exit==3 else 'rejected' if exit==1 else 'internal-error') and doc.get('schema_version')==1 and doc.get('operation',doc.get('command'))==operation and doc.get('exit_code')==exit and doc.get('model_proved') is False and doc.get('exhaustive') is False and isinstance(doc.get('accepted'),bool) and (doc['accepted']==(exit==0)) and (operation!='source-editor' or doc.get('artifact_admitted') is False) and (operation!='translate-rewrite' or exit!=0 or (doc.get('algorithm_checksum_verified') is False and doc.get('body_checksum_verified') is True and doc.get('exact_primary_generated_bytes') is False and doc.get('analysis',{}).get('accepted') is True and isinstance(doc.get('replacement'),dict) and isinstance(doc.get('artifact',{}).get('tla'),str)))
def plan_integrity(plan,source,body):
 try:
  assert plan['algorithm_checksum_verified'] is False
  encoded=source.encode(); span=plan['span']; start,end=span['start'],span['end']
  assert isinstance(start,int) and isinstance(end,int) and 0<=start<=end<=len(encoded)
  encoded[:start].decode();encoded[end:].decode()
  rebuilt=(encoded[:start]+plan['replacement'].encode()+encoded[end:]).decode()
  if plan['newlines_normalized']:
   rebuilt='\n'.join(rebuilt.replace('\r\n','\n').replace('\r','\n').splitlines())+'\n'
  assert rebuilt==plan['module_source']
  observed=body_of(plan['module_source'])
  assert observed.replace('\r\n','\n').replace('\r','\n')==body.replace('\r\n','\n').replace('\r','\n')
  checksum=format(zlib.crc32(''.join(body.replace('\r\n','\n').replace('\r','\n').split('\n')).encode()),'x')
  assert plan['translation_checksum']==checksum
  assert re.search(r'chksum\(tla\) = "'+checksum+r'"|checksum\(tla\+?\) = "'+checksum+r'"',plan['module_source'],re.I)
  return True
 except (KeyError,TypeError,ValueError,AssertionError,StopIteration):return False
def save(out,stem,p):
 capture={'exit_code':p.returncode}
 for stream in ['stdout','stderr']:
  data=getattr(p,stream);name=stem+'.'+stream+'.gz';(out/name).write_bytes(gzip.compress(data,mtime=0));capture[stream]=name;capture[stream+'_sha256']=sha(data)
 return capture
def compare(args):
 rows=[]
 for directory in DIRS:
  m=json.loads((directory/'primary/manifest.json').read_text());out=directory/'native';out.mkdir(exist_ok=True)
  assert len(m['rows'])==json.loads((directory/'selection.json').read_text())['selected']
  for row in m['rows']:
   source=(directory/'cases'/row['input']).read_bytes();assert sha(source)==row['input_sha256'];primary=directory/'primary'
   for capture in ['primary','sany','callback_reference_adapter','validator_reference_adapter']:
    if capture in row:
     for stream in ['stdout','stderr']:assert sha(gzip.decompress((primary/row[capture][stream]).read_bytes()))==row[capture][stream+'_sha256']
   result={'id':row['id'],'campaign':str(directory.relative_to(BASE)),'primary':row['primary']['status'],'primary_sany':row.get('sany',{}).get('exit_code')}
   with tempfile.TemporaryDirectory(prefix='dive-rewrite-native-') as tmp:
    work=Path(tmp);module=re.search(rb'MODULE (\w+)',source).group(1).decode();file=work/(module+'.tla');file.write_bytes(source)
    for profile in ['dive','tlc']:
     p=subprocess.run([str(args.native),'translate',file.name,'--rewrite','--checksums','tla','--replace-modified','--output','json']+(['--compat=tlc'] if profile=='tlc' else []),cwd=work,capture_output=True,timeout=30);doc=json.loads(p.stdout)
     assert file.read_bytes()==source
     assert integrity(doc,p.returncode,'translate-rewrite'),(row['id'],doc)
     capture=save(out,row['id']+'.'+profile,p)
     if doc['accepted']:
      assert doc['algorithm_checksum_verified'] is False and doc['body_checksum_verified'] is True and doc['exact_primary_generated_bytes'] is False
      assert plan_integrity(doc['replacement'],source.decode(),doc['artifact']['tla']),row['id']
      if row['primary']['status']!='translated' or row.get('sany',{}).get('exit_code')!=0:outcome='disagree'
      else:outcome='native-admitted-partial-checksum'
     else:outcome='native-refusal' if p.returncode==3 else 'rejection-accounted' if p.returncode==1 else 'disagree'
     result[profile]={'outcome':outcome,'capture':capture}
   if row['primary']['status']=='translated' and row.get('sany',{}).get('exit_code')==0:
    text=(primary/row['translated']['path']).read_bytes();assert sha(text)==row['translated']['sha256'];body=body_of(text.decode())
    p,doc=execute(args.example,{'source':source.decode(),'body':body,'normalize':True,'replace_modified':True});assert integrity(doc,p.returncode,'source-editor')
    capture=save(out,row['id']+'.editor',p)
    if doc['accepted']:
     plan=doc['plan'];assert plan_integrity(plan,source.decode(),body),row['id']
     # Only literal equality earns exact-primary-byte credit. All other accepted
     # edits retain nonproof diagnostics and the explicit partial-header difference.
     observed=plan['module_source'].encode();outcome='exact-primary-byte-match' if observed==text else 'partial-policy-edit'
     assert body_of(plan['module_source'])==body.replace('\r\n','\n').replace('\r','\n'),row['id']
     result['editor']={'outcome':outcome,'capture':capture,'primary_sha256':sha(text),'native_sha256':sha(observed),'body_origin':'pinned-primary-development-input','native_artifact_generation_credit':False}
    else:result['editor']={'outcome':'named-editor-refusal','capture':capture}
   else:result['editor']={'outcome':'uncredited-primary-rejection' if row['primary']['status']=='rejected' else 'uncredited-primary-invalid-artifact'}
   rows.append(result)
  print('Accounted',len(m['rows']),directory.name,flush=True)
 summaries={name:dict(collections.Counter(r[name]['outcome'] for r in rows)) for name in ['editor','dive','tlc']}
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'native_sha256':sha(args.native.read_bytes()),'example_sha256':sha(args.example.read_bytes()),'manifests':{str(d.relative_to(BASE)):sha((d/'primary/manifest.json').read_bytes()) for d in DIRS},'selected':len(rows),'model_proved':False,'family_certified':False,'algorithm_checksum_verified':False,'summary':summaries,'rows':rows}
 (BASE/'comparison.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(summaries));return 1 if any(s.get('disagree') for s in summaries.values()) else 0
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--native',type=Path,required=True);p.add_argument('--example',type=Path,required=True);raise SystemExit(compare(p.parse_args()))
