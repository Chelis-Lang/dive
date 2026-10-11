import hashlib,json,gzip,subprocess,collections
from pathlib import Path
import argparse
p=argparse.ArgumentParser()
for key in ['base','binary','out']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();BASE=a.base;BIN=a.binary;OUT=a.out;OUT.mkdir(exist_ok=False)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
rows=[]
for folder in ['']:
 current=BASE/folder;manifest=json.loads((current/'primary/manifest.json').read_text())
 for original in manifest['rows']:
  r=dict(original); r['output_id']=(folder.replace('../','').replace('/','_')+'_'+r['id']).strip('_')
  case=current/'cases'/(r['id']+'.tla');flag=next(iter(r['flags']),'');fairness={'-wf':'weak','-sf':'strong','-nof':'none','-wfNext':'next','':'declared'}.get(flag,flag);q=subprocess.run([str(BIN)],input=json.dumps({'source':case.read_text(),'options':{'fairness':{'declared':'Declared','weak':'Weak','strong':'Strong','none':'None','next':'Next'}.get(fairness,'Declared')}}).encode(),capture_output=True,timeout=30)
  for stream in ['stdout','stderr']:(OUT/(r['output_id']+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
  outcome='disagree';detail='';doc=None
  try:
   if flag not in ['', '-wf', '-sf', '-nof', '-wfNext']:
    assert r['reference_errors'] and not r['translated'] and b'Unknown command-line option' in gzip.decompress((current/'primary'/r['primary']['stdout']['path']).read_bytes())
    assert q.returncode==0; typed=json.loads(q.stdout); assert typed['model_proved']is False
    outcome='command-rejection-uncredited';detail='invalid reference CLI flag cannot form a typed native fairness request; no AST/CRC/exit-equivalence credit'
   else:
    assert q.returncode==0
    doc=json.loads(q.stdout);assert doc['model_proved']is False and doc['artifact_admitted']is False and doc.get('family_certified',False)is False and doc.get('exhaustive',False)is False and doc.get('result')not in ['internal-error','success','complete','violation']
    checksum={'Ok':doc['checksum']}if doc.get('accepted')else{'Err':doc.get('error',{})}
    if 'Ok'in checksum:
     a=checksum['Ok'];expected=(current/'primary'/(r['id']+'.translated.ast')).read_text()
     assert r['translated'] and not r['reference_errors'], 'reference translation rejected'
     assert a['serialized_ast']==expected, 'serialized bytes differ'
     assert a['checksum']==r['stages']['translated']['crc'], 'CRC differs'
     outcome='exact-raw-ast'if r.get('sany',{}).get('accepted')else'primary-invalid-raw-uncredited'
    else:
     error=checksum.get('Err',doc.get('selection_error',{}));diagnostic=error.get('diagnostic',error);code=diagnostic.get('code');detail=diagnostic.get('message','')
     if code=='Unsupported':outcome='named-refusal'
     elif code in ['Syntax','Lexical','Expression','Label']and(not r['translated']or r['reference_errors']):outcome='rejection-accounted'
     else:detail=str(error)
  except Exception as e:detail=str(e)
  rows.append({'id':r['output_id'],'outcome':outcome,'detail':detail,'document':doc})
summary=dict(collections.Counter(r['outcome']for r in rows));report={'schema_version':1,'selected':len(rows),'native_binary_sha256':sha(BIN),'primary_manifest_sha256':{f:sha(BASE/f/'primary/manifest.json')for f in ['']},'executed_script_sha256':sha(Path(__file__)),'family_certified':False,'summary':summary,'rows':rows}
(OUT/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(summary)
for r in rows:
 if r['outcome']=='disagree':print(r['id'],r['detail'])
raise SystemExit(int(bool(summary.get('disagree'))))
