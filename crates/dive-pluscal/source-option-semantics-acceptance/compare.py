"""Strict native raw serialization/control comparison; admission and proof remain separate."""
from pathlib import Path
import argparse,collections,gzip,hashlib,json,subprocess,tempfile
p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--cst',type=Path,required=True);p.add_argument('--out',default='native');a=p.parse_args();base=Path(__file__).resolve().parent;out=base/a.out;out.mkdir(exist_ok=False)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def run(path,args):return subprocess.run([str(path),*map(str,args)],capture_output=True,timeout=30)
def check(doc,row,folder,source):
 assert doc['model_proved'] is False
 if doc.get('result')=='selection-error':
  assert row['name']=='translation-block' and doc['diagnostic']['code']=='Unsupported'
  assert 'translation'not in doc and 'checksum'not in doc
  return 'named-selection-refusal',None
 assert doc['artifact_admitted'] is False
 if 'Err' in doc['translation'] or 'Err' in doc['checksum']:
  errors=[doc[k]['Err']['diagnostic']for k in ['translation','checksum']if 'Err'in doc[k]]
  assert all(e['code']in ['Unsupported','Resource','Label','Syntax','Expression','Lexical']for e in errors)
  allowed=(row['reference_errors'] or row['primary']['exit_code']!=0 or row['name']in ['cr-only','unicode-letter-suffix','unicode-space'])
  assert allowed,(row['id'],errors)
  if not row['reference_errors'] and row['name']=='cr-only':
   assert 'Ok'in doc['translation'] and 'Err'in doc['checksum']
   return 'generation-only-full-refused',doc['translation']['Ok']
  assert 'Err' in doc['translation'] and 'Err' in doc['checksum']
  return 'named-refusal' if any(e['code']in ['Unsupported','Resource']for e in errors) else 'rejection-accounted',None
 assert not row['reference_errors'] and row['translated'] and row['primary']['exit_code']==0
 artifact=doc['translation']['Ok'];checksum=doc['checksum']['Ok'];expected=(folder/(row['id']+'.translated.ast')).read_bytes()
 assert checksum['serialized_ast'].encode()==expected
 assert checksum['checksum']==row['stages']['translated']['crc']==row['header'][0]
 for context in [artifact['option_context'],checksum['option_context']]:
  wanted=row['context']['settings'];got=context['effective'];assert got['fairness']=={'':'Declared','nof':'None','wf':'Weak','sf':'Strong','wfNext':'Next'}[wanted['fairness']];assert got['label_root']==wanted['root'];assert got['insert_missing_labels']==wanted['label']
  assert bool(context['source_declaration'])==row['context']['found']
  if context['source_declaration']:
   declaration=context['source_declaration'];span=declaration['span'];assert source[span['start']:span['end']].startswith(b'PlusCal')
   for arg in declaration['arguments']:assert source[arg['span']['start']:arg['span']['end']].decode()==arg['text']
 return 'exact-raw-ast',artifact
rows=[];manifests={}
for name in ['primary-v2','scanner-variants-v2']:
 folder=base/name;manifest=json.loads((folder/'manifest.json').read_text());manifests[name]=sha(folder/'manifest.json')
 for row in manifest['rows']:
  sourcepath=folder/(row['id']+'.tla');assert sha(sourcepath)==row['source_sha256'];source=sourcepath.read_bytes();fairness='strong' if row['flags']==['-sf']else'declared'
  for profile in ['dive','tlc']:
   ident=row['id']+'_'+profile;q=run(a.binary,[sourcepath,'raw',fairness,profile]);(out/(ident+'.stdout.gz')).write_bytes(gzip.compress(q.stdout,mtime=0));(out/(ident+'.stderr.gz')).write_bytes(gzip.compress(q.stderr,mtime=0))
   try:
    assert q.returncode==0;doc=json.loads(q.stdout);outcome,artifact=check(doc,row,folder,source)
    if artifact:
     text=source.decode();terminator=text.rfind('====');native=text[:terminator]+artifact['tla']+'\n'+text[terminator:];(out/(ident+'.module.tla')).write_text(native)
     left=run(a.cst,[out/(ident+'.module.tla')]);right=run(a.cst,[folder/(row['id']+'.translated.tla')]);l=json.loads(left.stdout);r=json.loads(right.stdout);assert left.returncode==right.returncode==0
     if row['name']=='space-split':
      assert not l['accepted']and not r['accepted'];outcome='primary-invalid-raw-serialization'
     elif row['name']=='cr-only':
      assert not r['accepted'];outcome='generation-only-full-refused'
     else:assert l['accepted']and r['accepted']and l['tree']==r['tree']
     for stream,blob in [('native-cst',left.stdout),('primary-cst',right.stdout)]:(out/(ident+'.'+stream+'.gz')).write_bytes(gzip.compress(blob,mtime=0))
   except (AssertionError,KeyError,TypeError,ValueError)as error:outcome='disagree';print(ident,repr(error),flush=True)
   rows.append({'id':ident,'folder':name,'outcome':outcome,'stdout_sha256':hashlib.sha256(q.stdout).hexdigest()})
  if len(rows)%40==0:print('compared',len(rows),flush=True)
report={'schema_version':1,'script_sha256':sha(Path(__file__)),'native_binary_sha256':sha(a.binary),'cst_binary_sha256':sha(a.cst),'manifests':manifests,'selected':len(rows),'model_proved':False,'family_certified':False,'summary':dict(collections.Counter(r['outcome']for r in rows)),'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(report['summary']);raise SystemExit(bool(report['summary'].get('disagree')))
