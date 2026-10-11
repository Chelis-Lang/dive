#!/usr/bin/env python3
"""Read all portable P-elsif source/runtime bytes; preserve stage qualifications."""
import argparse,base64,gzip,hashlib,json,re
from collections import Counter
from pathlib import Path,PurePosixPath
HERE=Path(__file__).resolve().parent
SOURCE='37ace0360d4d9ae89f358161602f71898178b793'
def need(v,m):
 if not v:raise ValueError(m)
def sha(b):return hashlib.sha256(b).hexdigest()
def rel(n):
 p=PurePosixPath(n);need(not p.is_absolute() and '..'not in p.parts,'unsafe path');return p.as_posix()
def verify(base=HERE,binaries=None):
 d=json.loads((base/'delivery.json').read_bytes());need(d['source_commit']==SOURCE and d['all_consumers_closed'] and d['family_certified']is False and d['model_proved']is False,'qualification')
 raw=(base/rel(d['records_archive'])).read_bytes();need(sha(raw)==d['records_archive_sha256'],'pack identity');records={};size=0
 for row in json.loads(gzip.decompress(raw)):
  n=rel(row['path']);need(n not in records,'duplicate record');b=base64.b64decode(row['data'],validate=True);need(sha(b)==row['sha256']==d['files_sha256'].get(n),'record changed:'+n);records[n]=b;size+=len(b)
 need(set(records)==set(d['files_sha256']) and len(records)==d['records_count']==22105 and size==d['raw_bytes'],'complete census')
 for n,h in d['reports_sha256'].items():need(sha((base/'reports'/rel(n)).read_bytes())==h and sha(records[n.replace('__','/')])==h,'portable report readback')
 def doc(n):return json.loads(records[rel(n)])
 def packed(n):return gzip.decompress(records[rel(n)])
 producer=doc('native-first/producer.json');need(producer['source_commit']==SOURCE and len(producer['inputs'])==7291,'source authority')
 for n,e in producer['inputs'].items():need(sha(packed('native-first/'+rel(e['archive'])))==e['sha256'],'source archive:'+n)
 need(d['binary_sha256']==producer['binaries_sha256'],'binary authority')
 if binaries:
  for n,s in d['binary_sha256'].items():need(sha((binaries/rel(n)).read_bytes())==s,'binary changed:'+n)
 changes=json.loads((base/'packaging/changes.json').read_bytes());need(changes['gate_source_commit']==SOURCE and changes['runtime_retagged']is False and changes['rust_cargo_test_example_changes_after_gate']is False,'post-gate qualification')
 for name,e in changes['post_gate_verification_only_changes'].items():
  for key,field in [('before_sha256','archive'),('after_sha256','after_archive')]:need(sha(gzip.decompress((base/'packaging'/rel(e[field])).read_bytes()))==e[key],'guard/driver archival identity')
  need(producer['inputs'][name]['sha256']==e['before_sha256'],'prior verifier authority')
 replay=changes['post_gate_verification_only_changes']['crates/dive-pluscal/p-elsif-checksum-acceptance/replay.py'];need(replay['after_sha256']==sha(records['verification-driver-repair/replay-corrected.py']),'tracked driver equals proven external repair')
 prefix='crates/dive-pluscal/p-elsif-checksum-acceptance/';old='crates/dive-pluscal/nested-procedure-checksum-acceptance/'
 def source(n):return packed('native-first/source/'+rel(n)+'.gz')
 def raw_report(name,scope,total,summary):
  report=doc('native-first/'+name+'/report.json');need(report['selected']==len(report['rows'])==total and report['summary']==summary and report['family_certified']is False,'raw census:'+name)
  primary=json.loads(source(scope+'primary/manifest.json'));references={r['id']:r for r in primary['rows']};need(set(references)=={r['id']for r in report['rows']},'raw selection:'+name)
  for r in report['rows']:
   o=r['document'];need(o['model_proved']is False and o['artifact_admitted']is False and o.get('exhaustive',False)is False,'raw false proof')
   if r['outcome']in ['exact-raw-ast','primary-invalid-raw-uncredited']:
    crc=o['checksum']['Ok'];need(crc['serialized_ast'].encode()==source(scope+'primary/'+r['id']+'.translated.ast') and crc['checksum']==references[r['id']]['stages']['translated']['crc'],'raw byte/CRC')
   else:need(r['outcome']in ['named-refusal','rejection-accounted'],'raw outcome')
 raw_report('raw-new',prefix,32,{'exact-raw-ast':32})
 raw_report('raw-original',old,180,{'exact-raw-ast':137,'primary-invalid-raw-uncredited':2,'named-refusal':24,'rejection-accounted':17})
 raw_report('raw-while-tail',old+'while-tail/',4,{'exact-raw-ast':4})
 raw_report('raw-while-lines',old+'while-lines/',8,{'exact-raw-ast':8})
 def public(name,total,summary,scope=None,external=False):
  folder=(name if external else 'native-first/'+name)+'/';report=doc(folder+'report.json');need(report['selected']==len(report['rows'])==total and report['summary']==summary and report['model_proved']is False and report['family_certified']is False,'public census:'+name)
  for r in report['rows']:
   data=packed(folder+r['id']+'.native.stdout.gz');o=json.loads(data);need(sha(data)==r['native']['stdout_sha256'] and o['profile']==r['profile'] and o['model_proved']is False and o['exhaustive']is False and o['result']!='internal-error' and o['exit_code']==r['native']['exit_code'],'public claims')
   if r['outcome']=='admitted-exact-cst-sany-validator':
    need(o['accepted'] and o['exit_code']==0 and o['result']=='replacement' and o['algorithm_checksum_verified']is True and o['body_checksum_verified']is True,'admission')
    need(o['translation_options']==o['option_context']['effective'] and o['requested_translation_options']==o['option_context']['requested'],'effective context')
    if scope:
     ast=records[scope+'primary/'+r['source_id']+'.translated.ast']if external else source(scope+'primary/'+r['source_id']+'.translated.ast')
     need(o['replacement']['algorithm_checksum']['serialized_ast'].encode()==ast,'public stored header bytes')
    l=json.loads(packed(folder+r['id']+'.native-cst.stdout.gz'));rr=json.loads(packed(folder+r['id']+'.primary-cst.stdout.gz'));need(l['accepted'] and rr['accepted'] and l['tree']==rr['tree'],'whole CST')
    for tag in ['native','primary']:need(b'Semantic errors:'not in packed(folder+r['id']+'.'+tag+'-sany.stdout.gz'),'SANY')
    need('NO_DIVERGENCE'in r['validator']['configured'],'Validator')
   else:need(not o['accepted'] and o['exit_code']in [1,3] and o['result']in ['unsupported','rejected'] and 'replacement'not in o,'unsupported falsely admitted')
 public('public-new',64,{'admitted-exact-cst-sany-validator':64},prefix)
 public('public-original',360,{'admitted-exact-cst-sany-validator':274,'rejection-accounted':38,'named-public-refusal':36,'invalid-primary-uncredited':12},old)
 public('public-while-tail',8,{'admitted-exact-cst-sany-validator':8},old+'while-tail/')
 public('public-while-lines',16,{'admitted-exact-cst-sany-validator':8,'named-public-refusal':8},old+'while-lines/')
 for study,name,total in [('branch-study','native',4),('branch-reachability','raw',8)]:
  primary=doc(study+'/primary/manifest.json');raw=doc(study+'/'+name+'/report.json');need(raw['selected']==total and raw['summary']=={'exact-raw-ast':total},'supplemental raw census')
  refs={r['id']:r for r in primary['rows']};need(len(refs)==total,'supplemental complete selection')
  for r in raw['rows']:
   o=r['document'];crc=o['checksum']['Ok'];ref=refs[r['id']];need(o['model_proved']is False and o['artifact_admitted']is False and sha(records[study+'/cases/'+r['id']+'.tla'])==ref['input_sha256'] and crc['serialized_ast'].encode()==records[study+'/primary/'+r['id']+'.translated.ast'] and crc['checksum']==ref['stages']['translated']['crc'] and ref['sany']['accepted'],'supplemental source/AST/CRC')
  public(study+'/public',total*2,{'admitted-exact-cst-sany-validator':total*2},study+'/',True)
 first=doc('native-first/campaign.json');need(not first['passed'] and first['source_commit']==SOURCE and len(first['completed'])==10 and all(r['exit_code']==0for r in first['completed'][:-1]) and first['completed'][-1]['exit_code']!=0,'original driver failure preserved')
 repair=doc('verification-driver-repair/delta.json');before=records['verification-driver-repair/replay-before.py'];after=records['verification-driver-repair/replay-corrected.py'];need(sha(before)==repair['before_sha256'] and sha(after)==repair['after_sha256'] and after==before.replace(b"A/'models/report.json'",b"a.models/'report.json'").replace(b"A/'models'",b'a.models') and repair['tracked_source_unchanged'],'verification repair scope')
 remainder=doc('native-first/campaign-remainder.json');need(remainder['source_commit']==SOURCE and remainder['passed'] and len(remainder['completed'])==8 and all(r['exit_code']==0 for r in remainder['completed']),'complete repaired campaign')
 gate=doc('full-gate-first/report.json');need(gate['source_commit']==SOURCE and gate['passed'] and gate['all_stages']==len(gate['completed'])==51 and all(r['exit_code']==0 for r in gate['completed']),'full gate')
 logs=records['full-gate-first/45.log'];need(re.search(rb'1189 tests run:\s*1189 passed,\s*0 skipped',logs),'nextest complete inventory')
 for root,total in [('native-first/models-new/',64),('native-first/models-original/',64),('branch-reachability/models/',16)]:
  m=doc(root+'report.json');need(m['selected']==len(m['rows'])==total and m['worker_store_controls']==total*12 and not m['family_certified'],'finite selected settings')
  for r in m['rows']:
   need(len(r['settings'])==12 and r['outcome']=='finite-model-match','finite row')
   stream=packed(root+r['primary']['stdout']['path']);found=re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue\.',stream);need(len(found)==1 and all(r['primary_counts'][k]==int(v.replace(b',',b''))for k,v in zip(['generated','distinct','left_on_queue'],found[0])),'primary count authority')
   for setting in r['settings']:
    capture=setting['capture'];o=json.loads(packed(root+capture['stdout']['path']));violation=r['id'].endswith('.violation');need(o['verdict']==('invariant-violation'if violation else'success') and all(o['statistics'][k]==v for k,v in r['primary_counts'].items()) and capture['exit_code']==((12 if setting['profile']=='tlc'else 10)if violation else 0),'model class/count/exit')
   for v in r['independent_validation']:
    o=json.loads(packed(root+v['capture']['stdout']['path']));need((o['result']=='valid'if v['outcome']=='valid'else o['result']!='valid'),'independent witness')
 strict=0;canonical=0
 for root,total in [('native-first/replay-new-repaired/',320),('native-first/replay-original/',320),('branch-reachability/replay/',80)]:
  m=doc(root+'report.json');need(m['selected']==len(m['rows'])==total and m['qualified_canonical_is_not_tlc_validation'] and not m['family_certified'],'replay scope')
  for r in m['rows']:
   o=json.loads(packed(root+r['id']+'.stdout.gz'));need(o['result']==r['actual_result'] and r['exit_code']=={'valid':0,'unsupported':3,'invalid':1}[o['result']] and sha(packed(root+r['id']+'.stdout.gz'))==r['stdout_sha256'],'replay byte claims')
   need(('.'+r['profile']+'.')in r['id'],'replay profile switched')
   if r['control']!='original':need(o['result']!='valid','mutant falsely validated')
   elif o['result']=='valid':
    if r['profile']=='tlc':strict+=1
    else:canonical+=1
 need(strict==d['strict_saved_frame_valid_witnesses']==0 and canonical==d['qualified_canonical_valid']==72,'strict saved-frame certification gap')
 guides=doc('native-first/guides-final/report.json');need(guides['selected']==len(guides['rows'])==20 and all(r['outcome']=='admitted-no-model-proof'for r in guides['rows']),'actual guides')
 crc=doc('native-first/crc/report.json');need(crc['selected']==len(crc['rows'])==24 and all(r['model_proved']is False and r['outcome']in ['valid-unchanged','corruption-refused']for r in crc['rows']),'CRC controls')
 forged=doc('native-first/forgeries/report.json');need(forged['selected']==28 and forged['genuine_controls']==4 and forged['forged_controls']==24 and all(r['exit_code']==int(r['mutation']!='baseline')for r in forged['rows']),'forgeries')
 resource=doc('expansion-resource-controls/report.json');need(resource['source_commit']==SOURCE and resource['unchanged_limits'] and len(resource['rows'])==4,'resource producer')
 for r in resource['rows']:need('Normalization:Resource'in r['stdout'] and ':ok:'in r['stdout'],'after-parse resource refusal')
 activation=doc('branch-reachability/activated-calls.json');need(len(activation['rows'])==4 and all(r['middle_or_else_call_executed'] and 3 in r['visited_x']for r in activation['rows']) and not activation['strict_trace_validation_claim'],'branch activation, not certified replay')
 readback=json.loads((base/'packaging/source-readback.json').read_bytes());need(readback['gate_source_commit']==SOURCE and readback['gate_input_count']==7291 and readback['unchanged_gate_inputs']==7286 and readback['unchanged_rust_cargo_test_example'] and readback['historical_producer_retagged']is False,'post-gate source readback')
 need({r['path']for r in readback['post_gate_changes']}=={'crates/dive-pluscal/p-elsif-checksum-acceptance/README.md','crates/dive-pluscal/p-elsif-checksum-acceptance/replay.py','docs/decisions/0066-p-elsif-shared-header-checksums.md','docs/pluscal-p-elsif-checksums.md','scripts/check-native-parity-integration.py'},'compiled delta concealed')
 return {'records':len(records),'source_inputs':7291,'gate_tests':1189,'public_rows':472,'raw_rows':236,'model_settings':1728,'family_certified':False}
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--base',type=Path,default=HERE);p.add_argument('--binaries',type=Path);a=p.parse_args()
 try:print(json.dumps(verify(a.base,a.binaries),sort_keys=True))
 except (ValueError,KeyError,TypeError,AssertionError,FileNotFoundError)as error:raise SystemExit('P elsif delivery verification failed: '+str(error))
