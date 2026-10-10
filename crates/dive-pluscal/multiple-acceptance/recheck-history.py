#!/usr/bin/env python3
"""Fresh complete process/fairness comparisons without changing earlier raw evidence."""
import argparse,importlib.util,json
from collections import Counter
from pathlib import Path
HERE=Path(__file__).parent
spec=importlib.util.spec_from_file_location('multi_compare',HERE/'compare.py');compare=importlib.util.module_from_spec(spec);spec.loader.exec_module(compare)
def main(native):
 for folder in ['process-acceptance','process-acceptance/interactions','process-acceptance/subscripts','process-acceptance/names','fairness-acceptance','fairness-acceptance/positive','fairness-acceptance/interactions']:
  base=HERE.parent/folder;manifestpath=base/'fixtures/manifest.json';manifest=json.loads(manifestpath.read_text());rows=[]
  if manifest['oracle_sha256']!=compare.JAR_SHA or manifest['source_commit']!='94d0c5070852b37b801bb8998c436383ab7ee37b':raise ValueError('wrong pin')
  # Verify provenance against the historical scripts, not today's multi adapter.
  original=base
  while not (original/'capture.py').exists():original=original.parent
  if manifest['capture_script_sha256']!=compare.sha((original/'capture.py').read_bytes()) or manifest['adapter_sha256']!=compare.sha((original/'NormalizationProbe.java').read_bytes()):raise ValueError('historical capture mismatch')
  selection=json.loads((base/'selection.json').read_text())
  ids=[r['id'] for r in manifest['rows']]
  if len(ids)!=len(set(ids)) or set(ids)!={r['id'] for r in selection['rows']} or set(ids)!={p.stem for p in (base/'cases').glob('*.tla')}:raise ValueError('historical selection mismatch')
  for row in manifest['rows']:
   source=(base/'cases'/row['input']).read_bytes()
   if compare.sha(source)!=row['input_sha256']:raise ValueError('source mismatch')
   compare.verify(base/'fixtures',row['primary'])
   if 'sany' in row:compare.verify(base/'fixtures',row['sany'])
   outcome=compare.outcome(row,compare.execute(native,source,row['options']),source,base/'fixtures',native)
   rows.append({'id':row['id'],**outcome})
  report={'schema_version':1,'selected':len(rows),'manifest_sha256':compare.sha(manifestpath.read_bytes()),'comparator_sha256':compare.sha((HERE/'compare.py').read_bytes()),'native_sha256':compare.sha(native.read_bytes()),'script_sha256':compare.sha(Path(__file__).read_bytes()),'model_proved':False,'family_certified':False,'summary':dict(Counter(r['outcome'] for r in rows)),'rows':rows}
  (HERE/('previous-'+folder.replace('/','-')+'.json')).write_text(json.dumps(report,indent=2)+'\n');print(folder,report['summary'])
  if report['summary'].get('disagree'):raise ValueError('historical disagreement')
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--native',type=Path,required=True);main(parser.parse_args().native)
