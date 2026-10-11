"""Verify complete raw runtime-name recovery evidence and retained source authority."""
import gzip,hashlib,itertools,json
from pathlib import Path
root=Path.cwd();base=root/'conformance/probes/native-checkpoint-runtime-names'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
h=base/'before-authorities';m=load(h/'index.json')
for p,digest in m['matrices'].items():assert sha(h/'matrices'/p)==digest,p
for p,digest in m['sources'].items():assert hashlib.sha256(gzip.decompress((h/'sources'/(p+'.gz')).read_bytes())).hexdigest()==digest,p
p=base/'tlc-primary-final';m=load(p/'index.json');assert len(m['controls'])==15
seen=set()
for row in m['controls']:
 key=(row['case'],row['seed']);assert key not in seen;seen.add(key)
 assert row['exit']==12
 assert row['counts']==([37,37,11] if row['case']=='RuntimeChoose' else [6,4,0] if row['case']=='RuntimeDuplicates' else [4,4,0])
 for stream in ['stdout','stderr']:assert sha(p/row[stream])==row[stream+'_sha256']
assert seen==set(itertools.product(['RuntimeStrings','RuntimeChoose','RuntimeSequence','RuntimeSymView','RuntimeDuplicates'],[0,7,14]))
for directory,groups,controls in [('current-recovery',1440,7920),('original-recovery-regression',576,4032),('original-failure-regression',None,96)]:
 p=base/directory;m=load(p/'index.json');assert len(m['controls'])==controls
 if groups is not None:assert m['groups']==groups
 for source,digest in m['source_sha256'].items():assert sha(root/source)==digest,source
 for row in m['controls']:
  for stream in ['stdout','stderr']:assert sha(p/row[stream])==row[stream+'_sha256'],row
 for archive,digest in m['archives'].items():assert sha(p/archive)==digest
 if directory=='current-recovery':
  group={}
  for row in m['controls']:group.setdefault(row['stem'],{})[row['stage']]=(row['exit'],load(p/row['stdout']))
  assert len(group)==1440
  for stem,stages in group.items():
   expected={'full','pause','retain','tlc-reset','witness-valid'}
   if '-dive-' in stem:expected.add('witness-bad-init')
   assert set(stages)==expected,(stem,stages.keys())
   full,pause,retain,reset=[stages[k][1] for k in ['full','pause','retain','tlc-reset']]
   assert full['verdict']==retain['verdict']==reset['verdict']=='invariant-violation'
   assert pause['verdict']=='resource-limit' and pause['counterexample'] is None
   assert pause['checkpoint']['status']=='paused-incomplete'
   assert retain['counterexample']==reset['counterexample']==full['counterexample']
   assert retain['statistics']==full['statistics']
   assert reset['statistics']['generated']==full['statistics']['generated']-pause['statistics']['generated']+pause['statistics']['distinct']
   for v in [pause,retain,reset]:assert v['checkpoint']['transient_storage_cleanup_completed'] and not v['options']['checkpoint']['independent_certificate']
   if '-tlc-' in stem:
    assert all(v['options']['workers']['used']==1 for v in [full,pause,retain,reset])
    assert stages['witness-valid'][1]['result']=='unsupported'
    assert stages['witness-valid'][1]['unsupported']=='runtime string encounter order: the independent validator cannot establish names created outside the supplied trace (#124)'
   else:
    assert stages['witness-valid'][1]['result']=='valid' and stages['witness-bad-init'][1]['result']=='invalid'
p=base/'primary-correspondence-final';m=load(p/'index.json');assert len(m['controls'])==5
assert m['capture_script_sha256']==sha(base/'compare_primary.py')
for row in m['controls']:
 assert row['concrete_states_equal'] and row['exit']==12
 assert sha(root/row['primary_stdout'])==row['primary_sha256']
 for stream in ['stdout','stderr']:assert sha(p/row[stream])==row[stream+'_sha256']
p=base/'current-guide';m=load(p/'index.json')
assert sha(root/'docs/native-checkpoint.md')==m['guide_sha256']
for file,digest in m['evidence_sha256'].items():assert sha(p/file)==digest,file
assert m['rust_guide']['build_exit']==m['rust_guide']['run_exit']==0
print('Verified15 clean primary controls,1440 current recovery groups/7920 runs,576 original groups/4032 runs,96 failures and680 original source inputs; no family certificate')
