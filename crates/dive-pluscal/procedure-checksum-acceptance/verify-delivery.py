#!/usr/bin/env python3
"""Read back every portable producer/runtime record without Java or optional archives."""
import argparse,base64,gzip,hashlib,json
from collections import Counter
from pathlib import Path,PurePosixPath
HERE=Path(__file__).resolve().parent

def require(value,message):
 if not value:raise ValueError(message)

def digest(raw):return hashlib.sha256(raw).hexdigest()

def relative(name):
 p=PurePosixPath(name);require(not p.is_absolute() and '..' not in p.parts,'unsafe record path');return p.as_posix()

def verify(base=HERE,binaries=None):
 d=json.loads((base/'delivery.json').read_bytes())
 require(d['source_commit'] is None and d['all_consumers_closed'] and not d['family_certified'] and not d['model_proved'],'producer qualification')
 packed=(base/relative(d['records_archive'])).read_bytes();require(digest(packed)==d['records_archive_sha256'],'runtime pack changed')
 entries=json.loads(gzip.decompress(packed));records={};total=0
 for entry in entries:
  name=relative(entry['path']);require(name not in records,'duplicate runtime record')
  raw=base64.b64decode(entry['data'],validate=True);require(digest(raw)==entry['sha256']==d['files_sha256'].get(name),'runtime bytes: '+name)
  records[name]=raw;total+=len(raw)
 require(set(records)==set(d['files_sha256']) and len(records)==d['records_count'] and total==d['raw_total_bytes'],'complete runtime census')
 for epoch in ['native-third','native-fourth','full-gate']:
  producer=json.loads(records[epoch+'/producer.json']);require(producer['source_commit'] is None,'historical producer retagged')
  for name,expected in producer['source_sha256'].items():require(digest(gzip.decompress(records[epoch+'/source/'+relative(name)]))==expected,'source bytes: '+name)
 gate_producer=json.loads(records['full-gate/producer.json']);require(len(gate_producer['source_sha256'])==d['gate_source_inputs']==514,'complete gate source selection')
 packaging=json.loads((base/'packaging/changes.json').read_bytes());require(packaging['runtime_retagged'] is False and packaging['rust_cargo_test_example_changes_after_gate'] is False,'packaging qualification')
 for name,change in packaging['post_gate_verification_only_changes'].items():
  require(digest(gzip.decompress((base/'packaging'/relative(change['archive'])).read_bytes()))==change['before_sha256']==gate_producer['source_sha256'][name],'prior guard authority')
  require(digest(gzip.decompress((base/'packaging'/relative(change['after_archive'])).read_bytes()))==change['after_sha256'],'augmented guard authority')
 source_readback=json.loads((base/'packaging/source-readback.json').read_bytes());require(source_readback['gate_source_commit'] is None and source_readback['historical_producer_retagged'] is False and source_readback['gate_input_count']==514 and source_readback['unchanged_gate_inputs']==513 and source_readback['unchanged_rust_cargo_test_example'] is True,'delivery source qualification')
 require([r['path']for r in source_readback['post_gate_changes']]==['scripts/check-native-parity-integration.py'],'post-gate production changes')
 controls=json.loads((base/'packaging/controls.json').read_bytes());require(controls['mutation_controls']==14 and all(r['exit_code']==0 for r in controls['commands']),'packaging controls')
 for name,record in controls['records'].items():
  raw=(base/'packaging'/relative(record['archive'])).read_bytes();require(digest(raw)==record['archive_sha256'] and digest(gzip.decompress(raw))==record['raw_sha256'],'packaging log')
 require(b'Ran 14 tests' in gzip.decompress((base/'packaging'/controls['records']['mutations']['archive']).read_bytes()),'mutation inventory')
 reports={}
 for name,expected in d['reports_sha256'].items():
  raw=(base/'reports'/relative(name)).read_bytes();require(digest(raw)==expected,'report changed: '+name);reports[Path(name).stem]=json.loads(raw)
 def census(name,selected,expected):
  r=reports[name];require(r['selected']==selected and not r['family_certified'],'scope census')
  require(len(r['rows'])==len({x['id']for x in r['rows']})==selected,'row identity')
  require(r['summary']==expected and dict(Counter(x['outcome']for x in r['rows']))==expected,'disposition census');return r
 census('raw',132,{'exact-raw-ast':84,'named-refusal':30,'rejection-accounted':16,'primary-invalid-raw-uncredited':2})
 public=census('public',264,{'admitted-exact-cst-sany-validator':168,'rejection-accounted':36,'named-public-refusal':48,'invalid-primary-uncredited':12})
 require(public['model_proved'] is False and public['native_binary_sha256']==d['binary_sha256']['dive'],'public proof/epoch')
 for row in public['rows']:
  raw=gzip.decompress(records['native-fourth/public/'+row['id']+'.native.stdout.gz']);require(digest(raw)==row['native']['stdout_sha256'],'public stdout')
  doc=json.loads(raw);require(doc['model_proved'] is False and doc['exhaustive'] is False and doc['profile']==row['profile'] and doc['result']!='internal-error','public artifact/proof profile')
  if row['outcome']=='admitted-exact-cst-sany-validator':require(row['native']['exit_code']==0 and doc['accepted'] is True and doc['algorithm_checksum_verified'] is True and row['validator']=={'configured':'[NO_DIVERGENCE]','unconfigured':'[NO_DIVERGENCE]'},'artifact admission')
 models=reports['models'];require(models['selected']==32 and models['worker_store_controls']==384 and models['valid_witness_controls']==0 and not models['family_certified'],'model scope')
 settings=0
 for row in models['rows']:
  require(row['outcome']=='finite-model-match' and len(row['settings'])==12,'model row')
  require(len({(s['profile'],s['workers'],s['store'])for s in row['settings']})==12,'unique settings')
  for setting in row['settings']:
   require(setting['outcome']=='match','model mismatch');cap=setting['capture'];raw=gzip.decompress(records['native-fourth/models/'+cap['stdout']['path']]);require(digest(raw)==cap['stdout']['sha256'],'model stdout')
   doc=json.loads(raw);require(doc['verdict'] in ('success','invariant-violation'),'model outcome')
   for key,value in row['primary_counts'].items():require(doc['statistics'][key]==value,'model counts: '+key)
   settings+=1
 require(settings==384,'model settings census')
 replay=reports['replay'];require(replay['selected']==160 and replay['qualified_canonical_is_not_tlc_validation'] and replay['summary']=={'dive.valid':16,'dive.invalid':64,'tlc.unsupported':48,'tlc.invalid':32},'replay qualification')
 require(dict(Counter(r['profile']+'.'+r['actual_result']for r in replay['rows']))==replay['summary'],'replay census')
 for row in replay['rows']:
  raw=gzip.decompress(records['native-fourth/replay/'+row['id']+'.stdout.gz']);require(digest(raw)==row['stdout_sha256'],'replay stdout');doc=json.loads(raw);require(doc['result']==row['actual_result'],'replay result')
  if row['control']=='original' and row['profile']=='tlc':require(doc['result']=='unsupported' and row['exit_code']!=0,'strict saved frame not certified')
  elif row['control']!='original':require(doc['result']!='valid' and row['exit_code']!=0,'corrupt witness accepted')
 for name,selected in [('guides',20),('crc',24),('forgeries',28)]:require(reports[name]['selected']==selected and not reports[name]['family_certified'],'control scope')
 require(reports['forgeries']['genuine_controls']==4 and reports['forgeries']['forged_controls']==24,'forgery census')
 for row in reports['forgeries']['rows']:
  require(row['controls']==4 and ((row['mutation']=='baseline' and row['exit_code']==0 and row['outcome']=='genuine-reference-accepted') or (row['mutation']!='baseline' and row['exit_code']!=0 and row['outcome']=='all-forgeries-rejected')),'forged acceptance')
 for row in reports['crc']['rows']:
  require(row['model_proved'] is False and ((row['id'].endswith('.unchanged') and row['outcome']=='valid-unchanged' and row['validator']=='[NO_DIVERGENCE]') or (not row['id'].endswith('.unchanged') and row['outcome']=='corruption-refused' and row['validator']!='[NO_DIVERGENCE]')),'CRC corruption accepted')
 gate=reports['full-gate'];require(gate['passed'] and gate['all_stages']==len(gate['completed'])==49 and all(x['exit_code']==0 for x in gate['completed']),'full gate')
 require(b'1183 tests run: 1183 passed' in records['full-gate/43.log'] and b', 0 skipped' in records['full-gate/43.log'],'complete nextest inventory')
 for name,count in [('cm1',21),('vts',16)]:
  log=records['native-fourth/'+name+'-corrected.log'];require(b'workers [1, 4, 8]' in log and b'gate: '+str(count).encode()+b' of '+str(count).encode()+b' models run' in log and b'gate: passed' in log,'conformance widths')
 if binaries:
  for name,expected in d['binary_sha256'].items():require(digest((binaries/relative(name)).read_bytes())==expected,'binary changed: '+name)
 return {'records':len(records),'gate_sources':514,'gate_tests':1183,'public_rows':264,'model_settings':384,'strict_valid_witnesses':0,'qualified_canonical_valid':16,'family_certified':False}

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binaries',type=Path);a=p.parse_args();print(json.dumps(verify(binaries=a.binaries),sort_keys=True))
