"""Verify complete phase-L selections, byte-exact streams and historical sources."""
import hashlib,json,gzip
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
PROBE=ROOT/'conformance/probes/tlc-phase-l'
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def load(path):return json.loads(path.read_text())
def verify(base,manifest,selection,raw,count,compiler=True,historical=False):
 r=load(base/manifest);selected=load(base/selection)
 assert r['oracle_sha256']==PIN and r['reason']
 assert r['selection_sha256']==sha(base/selection)
 for name,digest in r['source_sha256'].items():
  p=base/name
  if historical and not p.exists() and name.startswith('capture-') and name.endswith('.py'):p=base/'capture.py'
  assert sha(p)==digest,p
 if compiler:
  c=r['compiler'];assert c['exit'] in (0,1)
  for channel in ['stdout','stderr']:assert sha(base/raw/('compiler.'+channel))==c[channel+'_sha256']
  if c['exit']:
   assert r['rows']==[];return
 assert len(r['rows'])==len(selected)==count
 assert [c['name'] for c in selected]==[c['name'] for c in r['rows']]
 assert len(set(c['name'] for c in selected))==count
 for row in r['rows']:
  assert row['timeout'] is False,row['name']
  for channel in ['stdout','stderr']:assert row[channel+'_sha256']==sha(base/raw/row['name']/channel)
  for p,digest in row.get('model_source_sha256',{}).items():assert sha(base/p)==digest,p
  if 'fields' in row:
   fields={}
   for line in (base/raw/row['name']/'stdout').read_text().splitlines():
    if '\t' in line:
     k,v=line.split('\t',1);assert k not in fields;fields[k]=v
   assert fields==row['fields']
def main():
 verify(PROBE,'slot-capture.json','slot-selection.json','raw-slots',55)
 verify(PROBE,'evaluation-capture.json','evaluation-selection.json','raw-evaluation',35)
 verify(PROBE,'model-capture.json','model-selection.json','raw-model',51)
 verify(PROBE,'tlc-capture.json','tlc-selection.json','raw-tlc',20,False)
 for name,count in [('historical-initial-evaluation',35),('historical-initial-model',27),('historical-first-model-runtime',27),('historical-before-error-codes',36),('historical-before-subscript-controls',36),('historical-before-dual-error-import',45),('historical-before-local-role',48)]:
  verify(PROBE/name,'capture.json','selection.json','raw',count,historical=True)
 verify(PROBE/'historical-first-tlc','capture.json','selection.json','raw',15,False,True)
 verify(PROBE/'historical-before-dual-error-import','tlc-capture.json','tlc-selection.json','raw-tlc',18,False,True)
 verify(PROBE/'historical-before-local-role','tlc-capture.json','tlc-selection.json','raw-tlc',19,False,True)
 for name in ['historical-before-state-if-refusal','historical-before-nonstuttering-order']:
  observation=load(PROBE/name/'observation.json')
  assert 'no' in observation['scope']
  for path,digest in observation['source_sha256'].items():assert sha(PROBE/name/path)==digest
  assert (PROBE/observation['raw_reference']).exists()
 for name in ['tlc-planning','tlc-tableau']:
  before=ROOT/'conformance/probes'/name/'historical-before-phase-l'
  matrix=load(before/'source-matrix.json')
  for path,digest in matrix['source_sha256'].items():assert sha(before/path)==digest,path
 observation=load(PROBE/'historical-opaque-construction/observation.json')
 for path,digest in observation['source_sha256'].items():assert sha(PROBE/'historical-opaque-construction'/path)==digest
 matrix=load(PROBE/'source-matrix.json')
 for path,digest in matrix['source_sha256'].items():assert hashlib.sha256(gzip.decompress((PROBE/'historical-delivery-source'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
 combined=load(ROOT/'conformance/probes/tlc-planning/combined-context-tableau-source-matrix.json')
 for path,digest in combined['source_sha256'].items():assert sha(ROOT/path)==digest,path
 print('verified55 structural,35 actual synthetic LiveCheck,51 complete bound-model rows,20 clean TLC runs and every retained historical failure/source')
if __name__=='__main__':main()
