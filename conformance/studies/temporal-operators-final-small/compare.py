"""Compare complete immutable selections without promoting any prior observation."""
import json,hashlib,sys
from pathlib import Path
root=Path(sys.argv[1]);base=Path(__file__).resolve().parent
oldbase=root/'conformance/studies/material-capture-final-small'
def report(b):
 candidates=[]
 for p in (b/'reports').glob('*.json'):
  m=json.loads(p.read_text())
  if len(m.get('models',[]))==140:candidates.append((p,m))
 assert len(candidates)==1,candidates
 return candidates[0]
oldpath,old=report(oldbase);newpath,new=report(base)
key=lambda r:(r['corpus'],r['model'])
assert [key(r)for r in old['models']]==[key(r)for r in new['models']]
assert old['golden_oracle_sha256']==new['golden_oracle_sha256']
rows=[];moves={}
for a,b in zip(old['models'],new['models']):
 transition=a['outcome']+' -> '+b['outcome'];moves[transition]=moves.get(transition,0)+1
 rows.append(dict(corpus=b['corpus'],model=b['model'],prior_outcome=a['outcome'],current_outcome=b['outcome'],prior_detail=a['detail'],current_detail=b['detail'],current_runs=b['runs'],worker_introspecting=b['worker_introspecting']))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
(base/'model-by-model.json').write_text(json.dumps(dict(schema_version=1,selected=140,prior_reference='conformance/studies/material-capture-final-small',prior_report_sha256=sha(oldpath),current_report_sha256=sha(newpath),prior_summary=old['summary'],current_summary=new['summary'],transitions=moves,models=rows,scope='Fresh final-only census; report and raw invocations retain all unsupported/timeout/error dispositions. No ordinary/full-family certification or aggregate-memory claim.'),indent=2)+'\n')
print(json.dumps(moves,sort_keys=True))
