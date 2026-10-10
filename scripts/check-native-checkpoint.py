"""Verify complete native checkpoint controls and immutable source history."""
import gzip, hashlib, json
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/native-checkpoint'
sha=lambda b:hashlib.sha256(b).hexdigest()
load=lambda p:json.loads(p.read_text())
d=load(BASE/'delivery.json')
for name,digest in d['source_sha256'].items():
 assert sha(gzip.decompress((BASE/'historical-delivery-source'/(name+'.gz')).read_bytes()))==digest,name
for name,digest in d['evidence_sha256'].items():assert sha((ROOT/name).read_bytes())==digest,name
m=load(BASE/'native/index.json');assert m['native_binary_sha256']==d['binary_sha256']['dive']
assert len(m['controls'])==576
assert len(m['archive_sha256'])==144
for name,digest in m['archive_sha256'].items():assert sha((BASE/'native'/name).read_bytes())==digest,name
selected={};seen=set()
for row in m['controls']:
 key=tuple(row[k] for k in ['case','profile','store','width','chaos','stage'])
 assert key not in seen;seen.add(key)
 for stream in ['stdout','stderr']:
  assert sha((BASE/'native'/row[stream]).read_bytes())==row[stream+'_sha256'],key
 selected.setdefault(key[:-1],{})[row['stage']]=(row['exit'],load(BASE/'native'/row['stdout']))
def observable(v):
 v=json.loads(json.dumps(v));v.pop('elapsed_ms',None);v.pop('checkpoint',None)
 for k in ['checkpoint','workers','profile']:v['options'].pop(k,None)
 return v
assert len(selected)==144
for key,stages in selected.items():
 assert set(stages)=={'full','pause','retain','tlc-reset'}
 full,pause,retain,reset=[stages[k] for k in ['full','pause','retain','tlc-reset']]
 assert pause[1]['verdict']=='resource-limit' and pause[1]['counterexample'] is None,key
 assert pause[1]['checkpoint']['status']=='paused-incomplete',key
 assert (retain[0],observable(retain[1]))==(full[0],observable(full[1])),key
 assert retain[1]['checkpoint']['status']=='recovered',key
 assert retain[1]['options']['workers']['used']==1,key
 assert retain[1]['options']['workers']['requested']==key[3],key
 assert retain[1]['options']['checkpoint']['independent_certificate'] is False,key
 assert reset[0]==full[0] and reset[1]['verdict']==full[1]['verdict'],key
 assert reset[1]['statistics']['distinct']==full[1]['statistics']['distinct'],key
 assert reset[1]['statistics']['generated']==full[1]['statistics']['generated']-pause[1]['statistics']['generated']+pause[1]['statistics']['distinct'],key
for family in ['tlc-planning','predecessor-spill']:
 h=ROOT/'conformance/probes'/family/'historical-before-checkpoint';m=load(h/'matrix.json')
 for name,digest in m['source_sha256'].items():assert sha(gzip.decompress((h/'sources'/(name+'.gz')).read_bytes()))==digest,name
m=load(BASE/'current-integration-source.json')
for name,digest in m['source_sha256'].items():assert sha((ROOT/name).read_bytes())==digest,name
print('verified576 original full/pause/recovery rows,144 archives, complete immutable source history and current integration')
