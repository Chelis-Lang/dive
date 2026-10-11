#!/usr/bin/env python3
"""Verify mandatory portable nested-header source and runtime evidence offline."""
import argparse,base64,gzip,hashlib,json
from collections import Counter
from pathlib import Path,PurePosixPath
HERE=Path(__file__).resolve().parent
FINAL='657eaebd844bc33fe90d3f7e2698eaa50d8868e5'
FIRST='623062fafb5aff1cb905e956caf33668d9b496c6'
PREFIX='crates/dive-pluscal/nested-procedure-checksum-acceptance/'
def require(v,m):
 if not v:raise ValueError(m)
def digest(v):return hashlib.sha256(v).hexdigest()
def relative(n):
 p=PurePosixPath(n);require(not p.is_absolute() and '..' not in p.parts,'unsafe record path');return p.as_posix()
def verify(base=HERE,binaries=None):
 d=json.loads((base/'delivery.json').read_bytes());require(d['runtime_source_commit']==FINAL and d['all_consumers_closed'] and not d['family_certified'] and not d['model_proved'],'producer qualification')
 packed=(base/relative(d['records_archive'])).read_bytes();require(digest(packed)==d['records_archive_sha256'],'runtime pack changed')
 records={};total=0
 for e in json.loads(gzip.decompress(packed)):
  n=relative(e['path']);require(n not in records,'duplicate runtime record');raw=base64.b64decode(e['data'],validate=True)
  require(digest(raw)==e['sha256']==d['files_sha256'].get(n),'runtime bytes: '+n);records[n]=raw;total+=len(raw)
 require(set(records)==set(d['files_sha256']) and len(records)==d['records_count'] and total==d['raw_total_bytes'],'complete runtime census')
 def doc(n):return json.loads(records[relative(n)])
 historical=doc('first-source/source.json');require(historical['source_commit']is None,'first draft retagged')
 for name,expected in historical['paths'].items():require(digest(gzip.decompress(records['first-source/'+relative(name)]))==expected,'first draft source')
 entry=doc('before-entry-work-repair/observation.json');require(entry['source_commit']is None and digest(gzip.decompress(records['before-entry-work-repair/checksum_transfers.rs.gz']))==entry['sha256'],'entry scan repair history')
 packaging=json.loads((base/'packaging/changes.json').read_bytes());require(not packaging['runtime_retagged'] and not packaging['rust_cargo_test_example_changes_after_gate'] and packaging['gate_source_commit']==FINAL,'packaging qualification')
 change=packaging['post_gate_verification_only_changes']['scripts/check-native-parity-integration.py']
 for key,name in [('before_sha256','archive'),('after_sha256','after_archive')]:require(digest(gzip.decompress((base/'packaging'/relative(change[name])).read_bytes()))==change[key],'guard archive authority')
 readback=json.loads((base/'packaging/source-readback.json').read_bytes());require(readback['gate_source_commit']==FINAL and not readback['historical_producer_retagged'] and readback['gate_input_count']==5912 and readback['unchanged_gate_inputs']==5908 and readback['unchanged_rust_cargo_test_example'],'source readback qualification')
 require({x['path'] for x in readback['post_gate_changes']}=={'crates/dive-pluscal/nested-procedure-checksum-acceptance/README.md','docs/decisions/0061-nested-procedure-header-vectors.md','docs/pluscal-nested-procedure-checksums.md','scripts/check-native-parity-integration.py'},'post-gate production changes')
 first=doc('native-first/producer.json');final=doc('native-repaired/producer.json');retry=doc('native-final/producer.json')
 for epoch,p,commit,size in [('native-first',first,FIRST,5414),('native-repaired',final,FINAL,5912)]:
  require(p['source_commit']==commit and len(p['inputs'])==size,'historical producer retagged/source census')
  for name,e in p['inputs'].items():require(digest(gzip.decompress(records[epoch+'/source/'+relative(name)]))==e['sha256'],'source bytes: '+name)
 require(change['before_sha256']==final['inputs']['scripts/check-native-parity-integration.py']['sha256'],'prior guard source')
 require(retry['source_commit']==FINAL and retry['inputs']==final['inputs'] and retry['binaries']==final['binaries'] and retry['source_archive_from']=='native-repaired/source' and retry['same_frozen_source_and_binaries_reused_after_enospc'],'retry epoch identity')
 require(d['binary_sha256']=={k:v['sha256']for k,v in final['binaries'].items()},'binary authority')
 require(doc('full-gate/producer.json')['source_commit']==FIRST and not doc('full-gate/report.json')['passed'] and doc('full-gate/interrupted.json')['gate_passed']is False,'interrupted gate credited')
 require(doc('full-gate-repaired/producer.json')['source_commit']==FINAL and not doc('full-gate-repaired/report.json')['passed'] and doc('full-gate-repaired/report.json')['completed'][-1]['exit_code']==110,'ENOSPC gate credited')
 require(doc('environment-failure657.json')['all_consumers_closed'],'failed environmental epoch not closed')
 campaign=doc('native-final/campaign.json');require(campaign['passed'] and campaign['source_commit']==FINAL and campaign['planned_stages']==len(campaign['completed'])==13 and all(r['exit_code']==0 for r in campaign['completed']),'complete runtime campaign')
 reports={}
 for name,sha in d['reports_sha256'].items():
  raw=(base/'reports'/relative(name)).read_bytes();require(digest(raw)==sha,'report changed');reports[Path(name).stem]=json.loads(raw)
 def census(name,n,expected):
  r=reports[name];require(r['selected']==n and not r['family_certified'],'scope census')
  require(len(r['rows'])==len({x['id']for x in r['rows']})==n,'row identity')
  require(r.get('summary')==expected==dict(Counter(x['outcome']for x in r['rows'])),'disposition census');return r
 def source(n):return gzip.decompress(records['native-repaired/source/'+PREFIX+n])
 for name,sub,n,expected in [('raw-original','',180,{'exact-raw-ast':137,'named-refusal':24,'rejection-accounted':17,'primary-invalid-raw-uncredited':2}),('raw-while-tail','while-tail/',4,{'exact-raw-ast':4}),('raw-while-lines','while-lines/',8,{'exact-raw-ast':8})]:
  r=census(name,n,expected);require(r['native_binary_sha256']==d['binary_sha256']['context'],'raw producer')
  primary=json.loads(source(sub+'primary/manifest.json'));pm={x['id']:x for x in primary['rows']};require(set(pm)=={x['id']for x in r['rows']},'primary selection')
  for row in r['rows']:
   native=json.loads(gzip.decompress(records['native-final/'+name+'/'+row['id']+'.stdout.gz']));require(native==row['document'] and native['model_proved']is False and native['artifact_admitted']is False,'raw assertion')
   if row['outcome']in('exact-raw-ast','primary-invalid-raw-uncredited'):
    ok=native['checksum']['Ok'];p=pm[row['id']];require(ok['serialized_ast']==source(sub+'primary/'+row['id']+'.translated.ast').decode() and ok['checksum']==p['stages']['translated']['crc'],'primary AST/CRC bytes')
    require(bool(p.get('sany',{}).get('accepted'))==(row['outcome']=='exact-raw-ast'),'invalid primary credited')
 for name,n,expected in [('public-original',360,{'admitted-exact-cst-sany-validator':274,'rejection-accounted':38,'named-public-refusal':36,'invalid-primary-uncredited':12}),('public-while-tail',8,{'admitted-exact-cst-sany-validator':8}),('public-while-lines',16,{'admitted-exact-cst-sany-validator':8,'named-public-refusal':8})]:
  r=census(name,n,expected);require(r['model_proved']is False and r['native_binary_sha256']==d['binary_sha256']['dive'] and r['cst_binary_sha256']==d['binary_sha256']['cst'],'public proof/epoch')
  for row in r['rows']:
   raw=gzip.decompress(records['native-final/'+name+'/'+row['id']+'.native.stdout.gz']);require(digest(raw)==row['native']['stdout_sha256'],'public stdout');nd=json.loads(raw)
   require(nd['model_proved']is False and nd['exhaustive']is False and nd['profile']==row['profile'] and nd['result']!='internal-error','public proof/profile')
   if row['outcome']=='admitted-exact-cst-sany-validator':require(row['native']['exit_code']==0 and nd['accepted']is True and nd['algorithm_checksum_verified']is True and row['validator']=={'configured':'[NO_DIVERGENCE]','unconfigured':'[NO_DIVERGENCE]'},'artifact admission')
   if row['outcome']=='named-public-refusal':require(row['native']['exit_code']!=0 and nd['accepted']is False and nd['result']=='unsupported','named refusal')
 def models(name,count,settings,refusals=0):
  r=reports[name];require(r['selected']==count and r['worker_store_controls']==settings and r['valid_witness_controls']==0 and not r['family_certified'],'model scope')
  require(len(r['rows'])==len({x['id']for x in r['rows']})==count,'model row identity');seen=0;refused=0
  for row in r['rows']:
   if row['outcome']=='named-generation-refusal':
    refused+=1;require(not row['settings'] and row['model_proved']is False,'refusal model credit');cap=row['generation'];raw=gzip.decompress(records['native-final/'+name+'/'+cap['stdout']['path']]);require(digest(raw)==cap['stdout']['sha256'],'refusal stdout');nd=json.loads(raw);require(cap['exit_code']==3 and nd['result']=='unsupported'and not nd['accepted']and not nd['model_proved'],'generation refusal');continue
   require(row['outcome']=='finite-model-match' and len(row['settings'])==12 and len({(s['profile'],s['workers'],s['store'])for s in row['settings']})==12,'model settings')
   for s in row['settings']:
    require(s['outcome']=='match','model mismatch');cap=s['capture'];raw=gzip.decompress(records['native-final/'+name+'/'+cap['stdout']['path']]);require(digest(raw)==cap['stdout']['sha256'],'model stdout');nd=json.loads(raw)
    require(nd['verdict']==('invariant-violation'if row['id'].endswith('.violation')else'success') and cap['exit_code']==((12 if s['profile']=='tlc'else 10)if row['id'].endswith('.violation')else 0),'model outcome')
    require(nd['options']['profile']==s['profile'] and nd['options']['workers']['requested']==s['workers'],'model profile/workers')
    for key,val in row['primary_counts'].items():require(nd['statistics'][key]==val,'model counts')
    seen+=1
   for v in row.get('independent_validation',[]):
    cap=v['capture'];raw=gzip.decompress(records['native-final/'+name+'/'+cap['stdout']['path']]);require(digest(raw)==cap['stdout']['sha256'],'independent stdout');nd=json.loads(raw)
    require(cap['exit_code']!=0 and nd['result']!='valid','strict witness forged')
    if v['control']=='original':require(v['outcome']=='pending-named-refusal'and nd['result']=='unsupported','saved-frame qualification')
  require(seen==settings and refused==refusals,'model census')
 models('models',64,768);models('while-models-bounded',12,96,4)
 r=reports['replay'];require(r['selected']==320 and r['qualified_canonical_is_not_tlc_validation'] and r['summary']=={'dive.valid':32,'dive.invalid':128,'tlc.unsupported':96,'tlc.invalid':64},'replay qualification');require(dict(Counter(x['profile']+'.'+x['actual_result']for x in r['rows']))==r['summary'],'replay census')
 for row in r['rows']:
  raw=gzip.decompress(records['native-final/replay/'+row['id']+'.stdout.gz']);require(digest(raw)==row['stdout_sha256'],'replay stdout');nd=json.loads(raw);require(nd['result']==row['actual_result'],'replay result')
  if row['control']=='original'and row['profile']=='tlc':require(nd['result']=='unsupported'and row['exit_code']!=0,'strict saved frame not certified')
  elif row['control']!='original':require(nd['result']!='valid'and row['exit_code']!=0,'corrupt witness accepted')
 for name,n in [('guides',20),('crc',24),('forgeries',28)]:require(reports[name]['selected']==n and not reports[name]['family_certified'],'control scope')
 for row in reports['forgeries']['rows']:require(row['controls']==4 and((row['mutation']=='baseline'and row['exit_code']==0)or(row['mutation']!='baseline'and row['exit_code']!=0)),'forged acceptance')
 for row in reports['crc']['rows']:require(row['model_proved']is False and((row['id'].endswith('.unchanged')and row['validator']=='[NO_DIVERGENCE]')or(not row['id'].endswith('.unchanged')and row['outcome']=='corruption-refused'and row['validator']!='[NO_DIVERGENCE]')),'CRC corruption accepted')
 require(doc('full-gate-final/producer.json')['inputs']==final['inputs'] and doc('full-gate-final/producer.json')['source_commit']==FINAL,'gate/source qualification')
 gate=reports['full-gate'];require(gate['passed']and gate['source_commit']==FINAL and gate['all_stages']==len(gate['completed'])==50 and all(x['exit_code']==0 for x in gate['completed']),'full gate')
 require(b'1187 tests run: 1187 passed'in records['full-gate-final/44.log']and b', 0 skipped'in records['full-gate-final/44.log'],'complete nextest inventory')
 for name,n,index in [('cm1',21,11),('vts',16,12)]:
  log=records['native-final/campaign-'+str(index).zfill(2)+'.log'];require(b'workers [1, 4, 8]'in log and b'gate: '+str(n).encode()+b' of '+str(n).encode()+b' models run'in log and b'gate: passed'in log,'conformance widths')
 if binaries:
  for n,s in d['binary_sha256'].items():require(digest((binaries/relative(n)).read_bytes())==s,'binary changed')
 return {'records':len(records),'runtime_sources':5912,'gate_tests':1187,'raw_rows':192,'public_rows':384,'model_settings':864,'strict_valid_witnesses':0,'qualified_canonical_valid':32,'family_certified':False}
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binaries',type=Path);a=p.parse_args();print(json.dumps(verify(binaries=a.binaries),sort_keys=True))
