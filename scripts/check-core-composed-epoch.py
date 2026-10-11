"""Read one closed receiving epoch without trusting current Git availability."""
import gzip,hashlib,json,posixpath,re,tarfile
from pathlib import Path
PRODUCER='05b3a1ef498ea1609c0413383b81bebb1b411749'
ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BASE = ROOT / 'conformance/probes/core-prefix-storage-integration/epochs' / PRODUCER
EXPECTED_INDEX_SHA = '9ef0dded2d1aa9da870c7c0baa8061535029e79acb8f77e7d3196a5a2641d79d'
sha=lambda b:hashlib.sha256(b).hexdigest()
def verify(base,expected_index_sha=EXPECTED_INDEX_SHA):
 assert sha((base/'index.json').read_bytes())==expected_index_sha
 index=json.loads((base/'index.json').read_text())
 assert index['producer']==PRODUCER and index['all_receiving_consumers_closed'] and index['family_certified']is False
 archives={}
 for filename,row in index['archives'].items():
  assert Path(filename).name==filename
  assert sha((base/filename).read_bytes())==row['sha256']
  with tarfile.open(base/filename,'r:gz')as archive:
   members=archive.getmembers();assert len({m.name for m in members})==len(members)
   assert all(m.isfile()and not m.name.startswith('/')and '..'not in Path(m.name).parts for m in members)
   blobs={m.name:archive.extractfile(m).read()for m in members}
  assert {n:sha(b)for n,b in blobs.items()}==row['members_sha256']
  archives[filename]=blobs
 assert set(archives)=={'source-and-gate.tar.gz','fresh-runtime.tar.gz','failed-qualification.tar.gz','receiving-drivers.tar.gz'}
 sg=archives['source-and-gate.tar.gz'];source=json.loads(sg['source-index.json']);runtime=archives['fresh-runtime.tar.gz']
 assert source['producer']==PRODUCER and source['tree']==index['source_tree'] and source['family_certified']is False
 assert len(source['inputs'])==index['source_inputs']==844
 actual={name:gzip.decompress(sg[row['archive']])for name,row in source['inputs'].items()}
 assert {name:sha(blob)for name,blob in actual.items()}=={name:row['sha256']for name,row in source['inputs'].items()}
 assert {row['archive']for row in source['inputs'].values()}=={n for n in sg if n.startswith('source/')}
 compiled=set(source['compiled_rust_sources']);assert len(compiled)==index['compiled_rust_sources']==479
 refs=[]
 for name in sorted(compiled):
  text=actual[name].decode();matches=list(re.finditer(r'\b(include_str|include_bytes|include)!\s*\(\s*"([^"\\]+)"\s*\)',text))
  assert len(matches)==len(list(re.finditer(r'\b(?:include_str|include_bytes|include)!\s*\(',text))),name
  for m in matches:
   target=posixpath.normpath(str(Path(name).parent/m[2]));assert not target.startswith('../')and target in actual
   refs.append({'source':name,'macro':m[1],'literal':m[2],'target':target,'sha256':sha(actual[target])})
 assert refs==source['embedded_references'] and len(refs)==index['literal_include_sites']==70
 assert len({r['target']for r in refs})==index['literal_include_inputs']==65
 gate=json.loads(sg['gate-receipt.json'])
 assert gate['producer']==PRODUCER and gate['source_index_sha256']==sha(sg['source-index.json'])
 assert len(gate['rows'])==index['gate_rows']==71 and all(r['exit']==0 for r in gate['rows'])
 assert [r['name']for r in gate['rows']]==['offline-'+str(i)for i in range(61)]+['complete-partitions','workspace-result-controls']+['stage-'+str(i)for i in range(8)]
 for row in gate['rows']:assert sha(sg[row['name']+'.log'])==row['log_sha256']
 supplement=json.loads(sg['ci-supplement-receipt.json']);assert supplement['producer']==PRODUCER and supplement['source_index_sha256']==sha(sg['source-index.json'])
 assert len(supplement['rows'])==index['ci_supplement_rows']==1 and supplement['rows'][0]['command']==['python3','-I','scripts/check-inductive-tuple-replay.py']
 for row in supplement['rows']:assert row['exit']==0 and sha(sg[row['name']+'.log'])==row['log_sha256']
 import shlex
 covered={tuple(r['command'])for r in gate['rows']+supplement['rows']}
 for line in actual['.github/workflows/ci.yml'].decode().splitlines():
  if line.strip().startswith('python3 '):assert tuple(shlex.split(line.strip()))in covered,line
 testmatch=re.search(rb'(\d+) tests run:\s*(\d+) passed(?: \(\d+ slow\))?, (\d+) skipped',sg['stage-2.log'])
 assert testmatch and testmatch[1]==testmatch[2] and int(testmatch[3])==0 and int(testmatch[1])==index['zero_skip_workspace_tests']
 closure=json.loads(runtime['closure.json']);steps=json.loads(runtime['steps.json'])
 assert closure['producer']==steps['producer']==PRODUCER and closure['all_consumers_closed'] and closure['family_certified']is False
 assert closure['source_index_sha256']==sha(sg['source-index.json']) and closure['steps']==steps['rows']
 assert len(steps['rows'])==index['runtime_commands'] and all(r['exit']==0 for r in steps['rows'])
 assert len({r['name']for r in steps['rows']})==len(steps['rows'])
 for row in steps['rows']:
  for stream in ['stdout','stderr']:assert sha(runtime[row['name']+'.'+stream])==row[stream+'_sha256']
 for name,count in [('disk-original-rows',1024),('disk-replay-rows',1040)]:
  receipt=json.loads(runtime[name+'/receipt.json']);assert receipt['producer']==PRODUCER and len(receipt['rows'])==count
  assert receipt['binary_sha256']==closure['binaries']['dive'] and receipt['family_certified']is False
  for row in receipt['rows']:
   for stream in ['stdout','stderr']:assert sha(runtime[name+'/'+row['case']+'.'+stream])==row[stream+'_sha256']
 witness=json.loads(runtime['disk-independent-witnesses.stdout']);assert len(witness['rows'])==144
 assert all(r['positive']=='valid'for r in witness['rows'])and sum(len(r['mutants'])for r in witness['rows'])==864
 assert all(m['validation']=='invalid'for r in witness['rows']for m in r['mutants'])
 for family,count in [('examples',21),('validation-smoke',16)]:
  paths=[n for n in runtime if n.startswith(family+'-reports/')and n.endswith('.json')];assert len(paths)==1
  report=json.loads(runtime[paths[0]])
  assert report['summary']['models_selected']==report['summary']['models']['agree']==count
  for row in report['models']:
   assert row['outcome']=='agree'and {r['workers']for r in row['runs']}=={1,4,8}
   assert all(r['outcome']=='agree'and not r['diffs']for r in row['runs'])
 failed=archives['failed-qualification.tar.gz']
 failure=json.loads(failed['dive-core-composed-offline-failure-0c35/gate-receipt.json'])
 assert failure['producer']=='0c35c76097ef01a2d6913a6f98e9e1336ff85fcd'and len(failure['rows'])==59
 assert all(r['exit']==0 for r in failure['rows'][:-1])and failure['rows'][-1]['name']=='offline-58'and failure['rows'][-1]['exit']==2
 assert 'dive-core-composed-before-serialized-git/retained-failure.json'in failed
 path_failure=json.loads(failed['dive-core-composed-runtime-before-path-normalization/steps.json']);assert path_failure['producer']==PRODUCER and path_failure['rows'][-1]['name']=='disk-original-runtime' and path_failure['rows'][-1]['exit']==1
 assert all(r['exit']==0 for r in path_failure['rows'][:-1])
 assert 'dive-core-composed-runtime-before-path-normalization/retained-failure.json'in failed
 return {'producer':PRODUCER,'inputs':len(actual),'workspace_tests':int(testmatch[1]),'runtime_commands':len(steps['rows']),'family_certified':False}
if __name__=='__main__':
 import argparse
 parser=argparse.ArgumentParser();parser.add_argument('--base',type=Path,default=DEFAULT_BASE);args=parser.parse_args()
 print(json.dumps(verify(args.base),sort_keys=True))
