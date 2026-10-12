"""Verify all reference epochs and stage bytes, retaining invalid/error dispositions."""
import gzip,hashlib,importlib.util,json,re,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
s=importlib.util.spec_from_file_location('capture_bytes',BASE/'capture_bytes.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m);records=m.load();sha=m.sha
for prefix,valid,translated in [('primary',218,222),('first-primary',204,207)]:
 manifest=json.loads(records[prefix+'/manifest.json']);assert manifest['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d';assert manifest['selected']==len(manifest['rows'])==242
 assert len({r['id']for r in manifest['rows']})==242
 cases='cases'if prefix=='primary'else'first-cases'
 for row in manifest['rows']:
  name=row['id'];assert sha(records[cases+'/'+name+'.tla'])==row['input_sha256'];assert sha(records[prefix+'/'+name+'.translated.tla'])==row['output_sha256']
  for tag,stage in row['stages'].items():
   for suffix in ['ast','json']:assert sha(records[prefix+'/'+name+'.'+tag+'.'+suffix])==stage[suffix+'_sha256']
   assert format(zlib.crc32(records[prefix+'/'+name+'.'+tag+'.ast']),'x')==stage['crc']
  if row['translated']and not row['reference_errors']:assert row['header'][0]==row['stages']['translated']['crc']
  for tag in ['primary','stages_process','sany','validator']:
   if tag not in row:continue
   for stream in ['stdout','stderr']:
    meta=row[tag][stream]
    if isinstance(meta,str):assert tag=='validator'and stream=='stdout'and gzip.decompress(records[prefix+'/'+name+'.validator.stdout.gz'])==meta.encode()
    else:assert sha(gzip.decompress(records[prefix+'/'+meta['path']]))==meta['sha256']
 assert sum(r.get('sany',{}).get('accepted')is True for r in manifest['rows'])==valid
 assert sum(r['translated']and not r['reference_errors']for r in manifest['rows'])==translated
for prefix in ['exploded','first-exploded']:
 manifest=json.loads(records[prefix+'/manifest.json']);assert manifest['native_credit']is False and manifest['family_certified']is False and manifest['selected']==len(manifest['rows'])==242
 cases='cases'if prefix=='exploded'else'first-cases'
 for row in manifest['rows']:
  name=row['id'];assert sha(records[cases+'/'+name+'.tla'])==row['input_sha256']
  for tag,stage in row['stages'].items():
   for suffix in ['ast','json']:assert sha(records[prefix+'/'+name+'.'+tag+'.'+suffix])==stage[suffix+'_sha256']
   assert format(zlib.crc32(records[prefix+'/'+name+'.'+tag+'.ast']),'x')==stage['crc']
  for stream in ['stdout','stderr']:
   meta=row[stream];assert sha(gzip.decompress(records[prefix+'/'+meta['path']]))==meta['sha256']
selection=json.loads((BASE/'selection.json').read_text());manifest=json.loads(records['primary/manifest.json']);assert manifest['selection_sha256']==sha((BASE/'selection.json').read_bytes());assert [r['id']for r in selection['rows']]==[r['id']for r in manifest['rows']]
new=[r for r in manifest['rows']if r['id'].startswith('ProcessHeader_')];assert len(new)==72
assert {r['id']for r in new if r.get('sany',{}).get('accepted')is False}=={'ProcessHeader_C_boundary_scoped_capture','ProcessHeader_P_boundary_scoped_capture'}
for prefix in ['cases','primary','exploded','first-cases','first-primary']:
 for p in (BASE/prefix).iterdir():
  if not p.is_file()or p.name=='manifest.json':continue
  assert sha(p.read_bytes())==sha(records[prefix+'/'+p.name]),p
print('Verified12258 portable capture records:two complete242 original/header and two separate242 exploded epochs; invalid/error rows uncredited, no native/model/family proof')

mutations=json.loads((BASE/'ownership-budget-mutations/report.json').read_text());assert len(mutations['rows'])==4
assert [r['exit_code']for r in mutations['rows']]==[101,101,101,0]
for row in mutations['rows']:
 for stream in ['stdout','stderr']:assert sha((BASE/'ownership-budget-mutations'/(row['mutation']+'.'+stream)).read_bytes())==row[stream+'_sha256']
 for name,digest in row['source_sha256'].items():assert sha((BASE/'ownership-budget-mutations'/(row['mutation']+'.'+name)).read_bytes())==digest
print('Three actual ownership/shared-budget source mutants fail; exact restored controls pass. First SyntaxError driver is retained without source edits/test credit.')

s=importlib.util.spec_from_file_location('nof_context',BASE/'nof_context.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m);nof=m.load()
setting=json.loads(nof['selection.json']);manifest=json.loads(nof['primary/manifest.json']);assert manifest['selected']==len(manifest['rows'])==56 and manifest['selection_sha256']==sha(nof['selection.json'])
assert [r['id']for r in setting['rows']]==[r['id']for r in manifest['rows']]
original={r['id']:r for r in json.loads(records['primary/manifest.json'])['rows']}
for row in manifest['rows']:
 name=row['id'];assert row['flags']==['-nof']and row['sany']['accepted']is True and row['primary']['exit_code']==0 and row['translated']and not row['reference_errors']
 assert sha(nof['cases/'+name+'.tla'])==row['input_sha256']==original[name]['input_sha256']
 assert sha(nof['primary/'+name+'.translated.tla'])==row['output_sha256']==original[name]['output_sha256']
 for tag,stage in row['stages'].items():
  for suffix in ['ast','json']:assert sha(nof['primary/'+name+'.'+tag+'.'+suffix])==stage[suffix+'_sha256']
  assert format(zlib.crc32(nof['primary/'+name+'.'+tag+'.ast']),'x')==stage['crc']
 assert row['header']==original[name]['header']and nof['primary/'+name+'.translated.ast']==records['primary/'+name+'.translated.ast']
 for tag in ['primary','stages_process','sany','validator']:
  for stream in ['stdout','stderr']:
   meta=row[tag][stream]
   if isinstance(meta,str):assert tag=='validator'and stream=='stdout'and gzip.decompress(nof['primary/'+name+'.validator.stdout.gz'])==meta.encode()
   else:assert sha(gzip.decompress(nof['primary/'+meta['path']]))==meta['sha256']
print('Verified955 separate NoF records/56 settings on unchanged sources: exact default-context outputs and original headers; no new unique sources/native/model proof.')
