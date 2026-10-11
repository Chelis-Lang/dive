#!/usr/bin/env python3
"""Semantic and byte-corruption controls for the mandatory portable delivery."""
import base64,gzip,hashlib,importlib.util,json,shutil,tempfile
from pathlib import Path
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('elsif_delivery',HERE/'verify-delivery.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
sha=lambda b:hashlib.sha256(b).hexdigest()
def check(case):
 with tempfile.TemporaryDirectory(prefix='dive-elsif-delivery-negative-')as tmp:
  base=Path(tmp);shutil.copytree(HERE/'reports',base/'reports');shutil.copytree(HERE/'packaging',base/'packaging');shutil.copy2(HERE/'delivery.json',base/'delivery.json');shutil.copy2(HERE/'runtime-records.json.gz',base/'runtime-records.json.gz');module.verify(base);d=json.loads((HERE/'delivery.json').read_bytes());rows=json.loads(gzip.decompress((HERE/d['records_archive']).read_bytes()))
  by={r['path']:r for r in rows}
  def replace(n,change):
   row=by[n];old=base64.b64decode(row['data']);doc=json.loads(old);change(doc);raw=json.dumps(doc,indent=2).encode();row['data']=base64.b64encode(raw).decode();row['sha256']=sha(raw);d['files_sha256'][n]=sha(raw);d['raw_bytes']+=len(raw)-len(old)
   report=n.replace('/','__')
   if report in d['reports_sha256']:(base/'reports'/report).write_bytes(raw);d['reports_sha256'][report]=sha(raw)
  if case=='wrong-producer':d['source_commit']='0'*40
  elif case=='false-family-certification':d['family_certified']=True
  elif case=='unclosed-consumers':d['all_consumers_closed']=False
  elif case=='duplicate-record':rows.append(dict(rows[0]));d['records_count']+=1
  elif case=='unsafe-record':rows[0]['path']='../escape'
  elif case=='missing-record':removed=rows.pop();d['files_sha256'].pop(removed['path']);d['records_count']-=1;d['raw_bytes']-=len(base64.b64decode(removed['data']))
  elif case=='gate-skipped':replace('full-gate-first/report.json',lambda x:x['completed'].pop())
  elif case=='failed-gate-credited':replace('full-gate-first/report.json',lambda x:x.update(passed=False))
  elif case=='failed-driver-erased':replace('native-first/campaign.json',lambda x:x.update(passed=True))
  elif case=='raw-crc-forged':replace('native-first/raw-new/report.json',lambda x:x['rows'][0]['document']['checksum']['Ok'].update(checksum='00000000'))
  elif case in ['public-false-proof','public-profile-switch','public-ast-forged']:
   n='native-first/public-new/report.json';report=json.loads(base64.b64decode(by[n]['data']));row=report['rows'][0];stream='native-first/public-new/'+row['id']+'.native.stdout.gz';old=base64.b64decode(by[stream]['data']);doc=json.loads(gzip.decompress(old))
   if case=='public-false-proof':doc['model_proved']=True
   elif case=='public-profile-switch':doc['profile']='tlc'if row['profile']=='dive'else'dive'
   else:doc['replacement']['algorithm_checksum']['serialized_ast']='forged'
   rawdoc=json.dumps(doc).encode();raw=gzip.compress(rawdoc,mtime=0);by[stream].update(data=base64.b64encode(raw).decode(),sha256=sha(raw));d['files_sha256'][stream]=sha(raw);d['raw_bytes']+=len(raw)-len(old);replace(n,lambda x:x['rows'][0]['native'].update(stdout_sha256=sha(rawdoc)))
  elif case=='model-primary-counts-forged':replace('native-first/models-new/report.json',lambda x:x['rows'][0]['primary_counts'].update(distinct=999999))
  elif case=='finite-selection-dropped':replace('native-first/models-new/report.json',lambda x:x['rows'].pop())
  elif case=='strict-witness-credit':d['strict_saved_frame_valid_witnesses']=1
  elif case=='canonical-as-tlc':replace('native-first/replay-new-repaired/report.json',lambda x:x['rows'][0].update(profile='tlc'))
  elif case=='resource-boundary-erased':replace('expansion-resource-controls/report.json',lambda x:x['rows'][0].update(stdout='48:ok:4311\n'))
  elif case=='branch-not-exercised':replace('branch-reachability/activated-calls.json',lambda x:x['rows'][0].update(visited_x=[1,1]))
  elif case=='corrupt-control-credited':replace('native-first/forgeries/report.json',lambda x:x['rows'][1].update(exit_code=0))
  elif case=='guide-not-executed':replace('native-first/guides-final/report.json',lambda x:x['rows'][0].update(outcome='planned'))
  elif case=='repair-changes-runtime':replace('verification-driver-repair/delta.json',lambda x:x.update(tracked_source_unchanged=False))
  else:raise ValueError('unknown case')
  archive=gzip.compress(json.dumps(rows,separators=(',',':')).encode(),mtime=0);(base/d['records_archive']).write_bytes(archive);d['records_archive_sha256']=sha(archive);(base/'delivery.json').write_text(json.dumps(d))
  try:module.verify(base)
  except (ValueError,KeyError,TypeError,FileNotFoundError):return
  raise AssertionError('invalid delivery accepted: '+case)
module.verify()
cases=['wrong-producer','false-family-certification','unclosed-consumers','duplicate-record','unsafe-record','missing-record','gate-skipped','failed-gate-credited','failed-driver-erased','raw-crc-forged','public-false-proof','public-profile-switch','public-ast-forged','model-primary-counts-forged','finite-selection-dropped','strict-witness-credit','canonical-as-tlc','resource-boundary-erased','branch-not-exercised','corrupt-control-credited','guide-not-executed','repair-changes-runtime']
for case in cases:check(case);print('rejected',case,flush=True)
print('Verified baseline and',len(cases),'complete-pack semantic/identity mutations; no family certification')
