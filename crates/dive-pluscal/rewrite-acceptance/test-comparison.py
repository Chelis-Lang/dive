#!/usr/bin/env python3
"""Negative integrity controls apply before any primary rejection/disposition."""
import argparse,copy,hashlib,importlib.util,json,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
 spec=importlib.util.spec_from_file_location('owned',BASE/'compare.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
 source=(BASE/'cases/P_fresh.tla').read_text();p,doc=m.execute(args.example,{'source':source,'body':'x == 1\n','normalize':True})
 assert m.integrity(doc,p.returncode,'source-editor');rows=[]
 for mutation in ['false-proof','exhaustive','missing-proof','missing-exhaustive','missing-schema','wrong-operation','internal-error','wrong-exit','missing-admission','missing-accepted','false-accepted']:
  changed=copy.deepcopy(doc)
  if mutation=='false-proof':changed['model_proved']=True
  elif mutation=='exhaustive':changed['exhaustive']=True
  elif mutation=='missing-proof':changed.pop('model_proved')
  elif mutation=='missing-exhaustive':changed.pop('exhaustive')
  elif mutation=='missing-schema':changed.pop('schema_version')
  elif mutation=='wrong-operation':changed['operation']='other'
  elif mutation=='internal-error':changed['result']='internal-error'
  elif mutation=='wrong-exit':changed['exit_code']=4
  elif mutation=='missing-admission':changed.pop('artifact_admitted')
  elif mutation=='missing-accepted':changed.pop('accepted')
  else:changed['accepted']=False
  assert not m.integrity(changed,p.returncode,'source-editor'),mutation
  rows.append({'operation':'source-editor','mutation':mutation,'outcome':'rejected'})
 with tempfile.TemporaryDirectory(prefix='dive-rewrite-integrity-') as tmp:
  file=Path(tmp)/'Rewrite_P.tla';file.write_text(source)
  cp=subprocess.run([str(args.native),'translate',file.name,'--rewrite','--checksums','tla','--output','json'],cwd=tmp,capture_output=True,timeout=30);actual=json.loads(cp.stdout)
  assert m.integrity(actual,cp.returncode,'translate-rewrite') and actual['accepted'],actual
  for key in ['model_proved','exhaustive','schema_version','command','exit_code','accepted','result']:
   changed=copy.deepcopy(actual)
   changed[key]={'model_proved':True,'exhaustive':True,'schema_version':2,'command':'other','exit_code':4,'accepted':False,'result':'internal-error'}[key]
   assert not m.integrity(changed,cp.returncode,'translate-rewrite'),key
   rows.append({'operation':'translate-rewrite','mutation':key,'outcome':'rejected'})
 for key in ['analysis','algorithm_checksum_verified','body_checksum_verified','exact_primary_generated_bytes']:
  changed=copy.deepcopy(actual)
  changed[key]={'analysis':{'accepted':False},'algorithm_checksum_verified':True,'body_checksum_verified':False,'exact_primary_generated_bytes':True}[key]
  assert not m.integrity(changed,cp.returncode,'translate-rewrite'),key
  rows.append({'operation':'translate-rewrite-admission','mutation':key,'outcome':'rejected'})
 for key in ['translation_checksum','module_source','replacement','span','algorithm_checksum_verified']:
  changed=copy.deepcopy(doc['plan'])
  changed[key]={'translation_checksum':'bad','module_source':source,'replacement':'bad','span':{'start':0,'end':999999},'algorithm_checksum_verified':True}[key]
  assert not m.plan_integrity(changed,source,'x == 1\n'),key
  rows.append({'operation':'source-editor-plan','mutation':key,'outcome':'rejected'})
 out={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'comparator_sha256':sha((BASE/'compare.py').read_bytes()),'example_sha256':sha(args.example.read_bytes()),'native_sha256':sha(args.native.read_bytes()),'controls':len(rows),'family_certified':False,'rows':rows}
 (BASE/'mutation-controls.json').write_text(json.dumps(out,indent=2)+'\n');print('Rejected',len(rows),'integrity mutations before primary disposition')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--example',type=Path,required=True);p.add_argument('--native',type=Path,required=True);main(p.parse_args())
