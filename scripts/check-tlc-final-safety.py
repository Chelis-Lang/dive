"""Verify clean safety-route controls and exact pre-route producer histories."""
from pathlib import Path
import hashlib,json,gzip,tarfile
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-final-safety'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
manifest=json.loads((BASE/'primary.json').read_text())
assert manifest['oracle']=='tlc-2026.10.06.014338'
assert manifest['flags']=={'workers':1,'seed':0,'fp':0,'lncheck':'final'}
assert len(manifest['rows'])==9 and len(set(manifest['selection']))==9
assert [r['model'] for r in manifest['rows']]==manifest['selection']
for row in manifest['rows']:
 for path,digest in row['files'].items(): assert sha(BASE/path)==digest,path
 assert int((BASE/'raw'/manifest['oracle']/(row['model']+'.exit')).read_text())==row['exit']
 assert f"STARTMSG {row['code']}:" in (BASE/'raw'/manifest['oracle']/(row['model']+'.stdout')).read_text()
for path,digest in manifest['history_files'].items():assert sha(BASE/path)==digest,path
assert manifest['bounded_native_only']==['SafetyInfinite']
history=BASE/'history-before-safety-route'
m=json.loads((history/'manifest.json').read_text())
assert m['parent']=='3464dd03024bd87364db5304137be114f44ba23d'
for row in m['matrices']:
 matrix=history/row['matrix'];assert sha(matrix)==row['matrix_sha256'],matrix
 old=json.loads(matrix.read_text())
 for source,digest in old['source_sha256'].items():
  assert hashlib.sha256(gzip.decompress((history/m['source_archive']/(source+'.gz')).read_bytes())).hexdigest()==digest,source
 current=json.loads((ROOT/row['matrix']).read_text())
 for source,digest in current['source_sha256'].items():assert sha(ROOT/source)==digest,source
print('Verified all nine clean safety-route captures and seven exact pre-route source matrices; resource stops and empty temporal obligations do not certify a temporal product')

history=BASE/'history-before-limits-receipt'
m=json.loads((history/'receipt.json').read_text())
assert m['status']=='passed' and m['workspace_tests']==865 and m['skipped_tests']==0
for path,digest in m['files'].items():assert sha(history/path)==digest,path
inputs=json.loads((history/'build-inputs.json').read_text())
source=json.loads((history/'source-sha256.json').read_text())
assert len(inputs)==m['build_related_input_count']==584
assert len(source)==m['source_evidence_input_count']==871
with tarfile.open(history/'source.tar.gz','r:gz') as archive:
 assert {entry.name for entry in archive.getmembers()}==set(source)
 for path,digest in source.items():
  assert hashlib.sha256(archive.extractfile(path).read()).hexdigest()==digest,path
for path,digest in inputs.items():assert source[path]==digest,path
print('Verified complete successful before-limits source/gate receipt; new limits controls are separate producer evidence')
