"""Verify the complete material selection and exact historical/current producers."""
from pathlib import Path
import gzip,hashlib,json,re
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/material-capture'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
FROZEN_MATERIAL=ROOT/'conformance/probes/temporal-operators/frozen-material-producer'
FROZEN_MATERIAL_MAP=load(FROZEN_MATERIAL/'manifest.json')['source_sha256']
def material_source(path,digest):
 if path.startswith('conformance/probes/material-capture/') and path not in FROZEN_MATERIAL_MAP:
  assert sha(ROOT/path)==digest,path
 else:
  assert FROZEN_MATERIAL_MAP[path]==digest,path
  assert hashlib.sha256(gzip.decompress((FROZEN_MATERIAL/'source'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
selection=load(BASE/'selection.json');assert len(selection)==16 and len({r['name']for r in selection})==16
primary=load(BASE/'clean-tlc/capture.json');assert primary['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert primary['selection_sha256']==sha(BASE/'selection.json')and primary['capture_source_sha256']==sha(BASE/'capture-clean.py')
assert [r['name']for r in primary['rows']]==[r['name']for r in selection]
refs={r['name']:r for r in primary['rows']}
# Native rows below belong to the immutable material producer, not this integration.
HISTORY=ROOT/'conformance/probes/native-parity-integration/history/material-runtime-source-authority'
PRODUCER=load(HISTORY/'record.json')
assert PRODUCER['source_commit']=='9b098e172d02abcf2dd3126faabf719ad5d1940a'
def material_source(path,digest):
 assert PRODUCER['source_sha256'][path]==digest,path
 assert hashlib.sha256(gzip.decompress((HISTORY/'sources'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
general_selection=load(BASE/'selection-general.json');assert len(general_selection)==2
general=load(BASE/'clean-general/capture.json');assert len(general['rows'])==2 and general['oracle_sha256']==primary['oracle_sha256']
assert general['selection_sha256']==sha(BASE/'selection-general.json')and general['capture_source_sha256']==sha(BASE/'capture-general.py')
refs.update({r['name']:r for r in general['rows']})
for row in primary['rows']+general['rows']:
 assert not row['timeout']and row['exit']==(13 if row['name'].endswith('Fails')else 0)
 for path,d in row['model_source_sha256'].items():assert sha(BASE/path)==d,path
 for stream in ['stdout','stderr']:assert sha(BASE/('clean-general'if row['name'].startswith('MatGeneralFunction')else'clean-tlc')/row['name']/stream)==row[stream+'_sha256']
# Every original archived source remains byte-exact. No historical native run
# acquires the current producer's source, field or verdict credit.
def archived(folder,sources):
 for path,d in sources.items():assert hashlib.sha256(gzip.decompress((folder/'source'/(path+'.gz')).read_bytes())).hexdigest()==d,path
origin=BASE/'frozen-origin-producer';om=load(origin/'manifest.json');assert om['commit']=='ea76af4ba6607e882a418ed08f4ab28006acb4b6';archived(origin,om['source_sha256'])
for path,count,widths,expected in [('native-before',32,[1,4],{'unsupported':32}),('native-general-before',4,[1,4],{'unsupported':4}),('development-open-refusals/native-current',64,[1,3,4,8],{'unsupported':32,'success':16,'liveness-violation':16}),('development-clippy-controls/native-current',64,[1,3,4,8],{'success':32,'liveness-violation':32})]:
 b=BASE/path;m=load(b/'capture.json');assert len(m['rows'])==count and m['source_and_binary_unchanged']is True
 assert [(r.get('name',r.get('model')),r['requested_workers'])for r in m['rows']]==[(r['name'],w)for r in (general_selection if path=='native-general-before'else selection)for w in widths]
 source=b if path in ['native-before','native-general-before']else b.parent;archived(source,m['source_sha256']);counts={}
 for row in m['rows']:
  name=row.get('name',row.get('model'));p=b/f"{name}-{row['requested_workers']}"
  for stream in ['stdout','stderr']:assert sha(p/stream)==row[stream+'_sha256']
  assert not row['timeout'];d=load(p/'stdout');counts[d['verdict']]=counts.get(d['verdict'],0)+1
  if d['verdict']=='unsupported':assert row['exit']==3 and d['counterexample']is None and d['options']['workers']['used']==0 and(d['tlc']is None or d['tlc']['verified']is False)
 assert counts==expected,(path,counts)
order_gap=BASE/'development-order-gap';og=load(order_gap/'manifest.json');archived(order_gap,og['source_sha256'])
assert load(order_gap/'gate-current/after.json')['all_steps_passed'] is False
assert load(order_gap/'gate-current/after.json')['source_and_binaries_unchanged'] is True
assert 'COMPLETED states [[Some("zeta")], [Some("alpha")]]' in (order_gap/'repro/output.log').read_text()
for folder,count in [('native-current',64),('native-general-current',8)]:
 old=load(order_gap/folder/'capture.json');assert len(old['rows'])==count and old['source_and_binary_unchanged']is True
 for path,d in old['source_sha256'].items():assert og['source_sha256'][path]==d,path
 for row in old['rows']:
  for stream in ['stdout','stderr']:assert sha(order_gap/folder/f"{row['model']}-{row['requested_workers']}"/stream)==row[stream+'_sha256']
  assert row['exit'] in [0,13] and not row['timeout']
assert 'useless conversion' in(BASE/'development-clippy-controls/clippy.log').read_text()
for current,selection_path,capture_path,current_selection,primary_path,expected in [(BASE/'native-current','selection.json','capture-native.py',selection,'clean-tlc',32),(BASE/'native-general-current','selection-general.json','capture-native-general.py',general_selection,'clean-general',4)]:
 m=load(current/'capture.json');assert m['source_and_binary_unchanged']is True
 assert m['selection_sha256']==sha(BASE/selection_path)and m['capture_source_sha256']==sha(BASE/capture_path)
 for path,d in m['source_sha256'].items():material_source(path,d)
 assert [(r['model'],r['requested_workers'])for r in m['rows']]==[(r['name'],w)for r in current_selection for w in [1,3,4,8]]
 counts={}
 for row in m['rows']:
  p=current/f"{row['model']}-{row['requested_workers']}";d=load(p/'stdout');ref=refs[row['model']]
  for stream in ['stdout','stderr']:assert sha(p/stream)==row[stream+'_sha256']
  assert not row['timeout']and row['exit']==ref['exit']==d['tlc']['exit_status']
  assert d['schema_version']==2 and d['tlc']['verified']is True and d['tlc']['code']in ref['message_codes']
  assert d['options']['workers']['requested']==d['options']['workers']['used']==row['requested_workers']
  assert d['options']['ordinary_temporal_schedule_supported']is False
  assert d['execution']['capture_completed']is True and d['execution']['schedule']=='final-only'and d['execution']['selection']=='tlc'
  for field in ['configured_property_names','expanded_property_names','role_names']:assert d['execution'][field]==['Live']
  raw=(BASE/primary_path/row['model']/'stdout').read_text();s=re.search(r'@!@!@STARTMSG 2199:0 @!@!@\n(\d+) states generated, (\d+) distinct states found',raw);depth=re.search(r'The depth of the complete state graph search is (\d+)',raw);assert s and depth
  for field,value in [('generated',s[1]),('distinct',s[2]),('depth',depth[1])]:assert d['statistics'][field]==int(value),(row['model'],field)
  counts[d['verdict']]=counts.get(d['verdict'],0)+1
  if ref['exit']==13:
   c=d['counterexample'];assert d['verdict']=='liveness-violation'and c['validation']=='valid'and c['ending']=='stuttering'
   pinned=[s.splitlines()[2]=='x = TRUE' for s in raw.split('@!@!@STARTMSG 2217:4 @!@!@')[1:]]
   assert[c['states'][i]['values']['x']for i in range(len(c['states']))]==pinned
   assert c['trace']['properties']==['Live']and c['trace']['ending']=={'kind':'stuttering'}
  else:assert d['verdict']=='success'and d['counterexample']is None
 assert counts=={'liveness-violation':expected,'success':expected}
example=BASE/'user-example-controls';e=load(example/'receipt.json');assert e['compile_exit']==e['run_exit']==0 and e['source_and_binary_unchanged']is True
assert sha(BASE/'user-example.rs')==e['source_sha256']
for path,d in e['crate_source_sha256'].items():material_source(path,d)
for path,d in e['files'].items():assert sha(example/path)==d,path
assert(example/'stdout').read_text().strip()=='["Live"]'
# Current matrix checks current bytes; runtime observations retain their frozen producer.
for path,d in load(BASE/'source-matrix.json')['source_sha256'].items():assert sha(ROOT/path)==d,path
print('Verified18 primary/72 native material pairs and all36 prior refusals/64 first-attempt outcomes; fresh replay and fixed50k/128 controls, no corpus/family/ordinary credit')
