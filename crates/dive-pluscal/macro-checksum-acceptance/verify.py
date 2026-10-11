"""Read back all clean primary inputs and archived stage/stream identities."""
from pathlib import Path
import gzip,hashlib,json
root=Path(__file__).resolve().parent
sha=lambda data:hashlib.sha256(data).hexdigest()
count=0;streams=0
for folder in ['', 'boundary','layout','regression','api-regression','ownership','insertion','insertion-old','closing-line','inline','marker','marker-extra','marker-single','guide']:
 base=root/folder;manifest=json.loads((base/'primary/manifest.json').read_text())
 assert manifest['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
 for row in manifest['rows']:
  name=row['id'];assert sha((base/'cases'/(name+'.tla')).read_bytes())==row['input_sha256']
  assert sha((base/'primary'/(name+'.translated.tla')).read_bytes())==row['output_sha256']
  for tag,stage in row['stages'].items():
   for suffix in ['ast','json']:assert sha((base/'primary'/(name+'.'+tag+'.'+suffix)).read_bytes())==stage[suffix+'_sha256']
  for tag in ['primary','stages_process','sany','validator']:
   if tag not in row:continue
   for stream in ['stdout','stderr']:
    meta=row[tag][stream]
    if isinstance(meta,str):
     assert tag=='validator'and stream=='stdout'
     assert gzip.decompress((base/'primary'/(name+'.validator.stdout.gz')).read_bytes())==meta.encode()
    else:assert sha(gzip.decompress((base/'primary'/meta['path']).read_bytes()))==meta['sha256']
    streams+=1
  count+=1
assert count==350
print('Verified350 sources (348 selection plus2 exact guides), all primary artifacts and',streams,'raw streams; no artifact/model/family certification implied.')
