#!/usr/bin/env python3
"""Separate corrected syntax; never overwrite original failed modifier controls."""
import json,re
from pathlib import Path
base=Path(__file__).resolve().parent
out=base/'corrected-modifiers';(out/'cases').mkdir(parents=True,exist_ok=True)
rows=[]
for row in json.loads((base/'selection.json').read_text())['rows']:
 source=(base/'cases'/(row['id']+'.tla')).read_text()
 fixed=re.sub(r'\b(\w+)([+-]):',r'\1: \2 ',source)
 if source==fixed:continue
 name=row['id'].replace('FairProcedure_','FairProcedureModifier_')
 (out/'cases'/(name+'.tla')).write_text(fixed.replace(row['id'],name))
 rows.append({'id':name,'options':row['options'],'original_id':row['id']})
(out/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n')
for name in ('capture.py','NormalizationProbe.java','compare.py'):(out/name).write_bytes((base/name).read_bytes())
print('Separate corrected selection:',len(rows))
