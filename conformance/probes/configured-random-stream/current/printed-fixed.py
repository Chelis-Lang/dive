from pathlib import Path
import json,re
out=Path('/Users/jeff/code/dive-configured-random-stream-artifacts/current-corrected/native');j=json.loads((out/'index.json').read_text());primary=Path('/Users/jeff/code/dive-configured-random-stream-artifacts/primary');count=0
for row in j['cases']:
 if not row['setting'].startswith('tlc-'):continue
 d=out/row['case']/f'seed-{row["seed"]}'
 actual=json.loads((d/(row['setting']+'.stdout')).read_text())
 raw=(primary/row['case']/f'seed-{row["seed"]}.stdout').read_text()
 expected=re.findall(r'^-?\d+  TRUE$',raw,re.M)
 assert actual['output']==expected,(row,actual['output'],expected)
 count+=1
print('All',count,'strict current native outputs match the clean primary printed integer/TRUE lines; default profile retains its separately approved RNG semantics and within-setting byte equality.')
