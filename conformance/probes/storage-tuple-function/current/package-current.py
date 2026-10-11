from pathlib import Path
import os,subprocess,json,hashlib,shutil,gzip
REQ={'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target','CARGO_BUILD_JOBS':'2','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache'}
for k,v in REQ.items():assert os.environ.get(k)==v,(k,os.environ.get(k))
r=Path('/Users/jeff/code/dive-trace-representation-work');a=Path('/Users/jeff/code/dive-storage-tuple-function-artifacts');b=r/'conformance/probes/storage-tuple-function';out=b/'current';out.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
g=json.loads((a/'corrected-full-gate/index.json').read_text());producer=json.loads((a/'current-binaries/producer.json').read_text());runtime=json.loads((a/'current-runtime/receipt.json').read_text());guides=json.loads((a/'current-guide-gates/receipt.json').read_text())
assert producer['source_commit']==g['producer_commit']==runtime['runtime_producer']['source_commit']==guides['runtime_producer']['source_commit']==subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip()
assert len(g['stages'])==8 and all(c['exit']==0 for s in g['stages'] for c in s['commands'])
assert '1167 tests run: 1167 passed, 0 skipped' in(a/'corrected-full-gate/nextest-0.log').read_text()
assert len(runtime['rows'])==192 and len(runtime['validation'])==768 and len(runtime['old_archives'])==12
assert all(q['exit']==0 for q in guides['rows'])
for name,digest in producer['binaries_sha256'].items():assert sha(a/'current-binaries'/name)==digest
for name in ['corrected-full-gate','current-runtime']:
 shutil.copytree(a/name,out/('gate'if name=='corrected-full-gate'else'runtime'))
shutil.copytree(a/'current-guide-gates',out/'guides',ignore=shutil.ignore_patterns('storage_tuple_function'))
for p in ['corrected-full-gate-driver.py','build-corrected-current.py','runtime-driver.py','guide-gates-driver.py','package-current.py','unchanged-mathematical-authority.json','corrected-fixture-source-delta.json','current-gate-timing.json','corrected-preflight.log','current-release-build.log']:
 shutil.copy2(a/p,out/p)
source=json.loads((out/'gate/source/index.json').read_text())
for name,row in source['inputs'].items():
 raw=gzip.decompress((out/'gate/source'/row['archive']).read_bytes());assert hashlib.sha256(raw).hexdigest()==row['sha256'];assert raw==(r/name).read_bytes();assert raw==subprocess.check_output(['git','show',producer['source_commit']+':'+name],cwd=r)
files={str(p.relative_to(out)):sha(p)for p in out.rglob('*')if p.is_file()}
receipt={'schema_version':1,'runtime_producer':producer,'source_inputs':len(source['inputs']),'files_sha256':files,'gate':{'tests':1167,'skipped':0,'all8passed':True,'producer_commit':g['producer_commit'],'total_seconds':g['total_seconds'],'jobs':2,'test_threads':2,'time_limits':'Unchanged ci-profile per-test bounds. Local two-thread duration on a shared M3 is not Linux15min completion evidence.'},'scope':'Structural storage representation only;192 fresh check/pause/recovery triples,192 Valid original words/576 Invalid mutants,12 actual old CP01 archive refusals; literal/fcn/key/FP and current original corruptions checked. Application error EC1000/2103 metadata remains verified:false. No archive authenticity/reachability/completeness or whole-store memory/feature-family certification.','history':'dc0f failed full gate retained with1160/1167+seven coherent-current-header failures; corrected fixture migration mutation proves payload admission. No old producer/raw observations retagged.'}
(out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print('PACKAGED',len(files),'receipt',sha(out/'receipt.json'))
