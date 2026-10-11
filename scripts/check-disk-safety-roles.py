#!/usr/bin/env python3
"""Read back bounded disk-role source and consumers; hashes are not certificates."""
import argparse,copy,gzip,hashlib,itertools,json,pathlib,posixpath,re
PRODUCER='7fae387c11675b6a72119cc0013dd18f4eac33cb'
LIMIT=64*1024*1024
REFUSED={'PrintedProperty','PrintedSpecification','AssumePrintSafety','FairSpecification','TemporalProperty','QuantifiedProperty','WrapperSpecification'}
WORDS={'ActionProperty','AlwaysProperty','ConjunctionProperty','InstanceProperty','LetProperty','PropertyAlias'}
ALLOWLIST=['elapsed_ms','options.workers','options.seen_disk','seen_storage']
def safe(base,name):
 p=pathlib.PurePosixPath(name)
 assert not p.is_absolute() and p.parts and all(n not in ('.','..') for n in p.parts)
 result=base.joinpath(*p.parts);assert not result.is_symlink() and result.resolve().is_relative_to(base.resolve());return result
def read(path):
 assert path.stat().st_size<=LIMIT,path
 return path.read_bytes()
def sha(data):return hashlib.sha256(data).hexdigest()
def load(path):return json.loads(read(path))
def unpack(path):
 with gzip.open(path,'rb') as f:data=f.read(LIMIT+1)
 assert len(data)<=LIMIT,path
 return data
def stable(doc,disk=False,workers=False):
 result=copy.deepcopy(doc);result.pop('elapsed_ms')
 if disk:result.pop('seen_storage',None);result['options'].pop('seen_disk')
 if workers:result['options'].pop('workers')
 return result
def rows(base,directory,receipt):
 result={}
 for row in receipt['rows']:
  name=row['case'];assert name not in result
  for channel in ('stdout','stderr'):
   assert sha(read(safe(base,f'{directory}/{name}.{channel}')))==row[channel+'_sha256']
  result[name]=load(safe(base,f'{directory}/{name}.stdout'))
 return result

def original_git_blobs(base):
 proof_path=base/'producer-git-objects/index.json'
 assert sha(read(proof_path))=='37e3742a4b63089ba0a9098c72064075e953c0c168af066f58d0999f502f0aa2'
 proof=load(proof_path);assert proof['producer']==PRODUCER
 objects={}
 for oid,row in proof['objects'].items():
  packed=read(base/'producer-git-objects/objects'/(oid+'.gz'));assert sha(packed)==row['sha256']
  data=unpack(base/'producer-git-objects/objects'/(oid+'.gz'));kind=row['kind'];assert kind in ['commit','tree']
  assert hashlib.sha1((kind+' '+str(len(data))+'\0').encode()+data).hexdigest()==oid;objects[oid]=(kind,data)
 kind,commit=objects[PRODUCER];assert kind=='commit';tree=commit.splitlines()[0].removeprefix(b'tree ').decode();assert tree==proof['tree']
 parsed={}
 def entries(oid):
  if oid in parsed:return parsed[oid]
  kind,data=objects[oid];assert kind=='tree';result={};i=0
  while i<len(data):
   split=data.index(b' ',i);end=data.index(b'\0',split);mode=data[i:split].decode();name=data[split+1:end].decode();child=data[end+1:end+21].hex()
   assert len(child)==40 and name not in result;result[name]=(mode,child);i=end+21
  assert i==len(data);parsed[oid]=result;return result
 for name,blob in proof['blob_oid'].items():
  parts=pathlib.PurePosixPath(name).parts;assert not pathlib.PurePosixPath(name).is_absolute() and '..' not in parts;oid=tree
  for part in parts[:-1]:mode,oid=entries(oid)[part];assert mode=='40000'
  mode,actual=entries(oid)[parts[-1]];assert mode in ['100644','100755'] and actual==blob,name
 return proof['blob_oid']

def verify(base,source_root=None,binary_root=None):
 index=load(base/'delivery-index.json')
 assert index['runtime_commit']==PRODUCER and index['family_certified'] is False
 assert sha(read(pathlib.Path(__file__)))==index['verifier_sha256']
 actual={p.relative_to(base).as_posix() for p in base.rglob('*') if p.is_file() and p.name!='delivery-index.json'}
 assert actual==set(index['payload_sha256'])
 for name,digest in index['payload_sha256'].items():assert sha(read(safe(base,name)))==digest,name
 source=load(base/'source-index.json');assert source['producer']==PRODUCER and len(source['inputs'])==1347
 blobs=original_git_blobs(base);assert set(blobs)==set(source['inputs'])
 for name,row in source['inputs'].items():
  data=unpack(safe(base,row['archive']));assert sha(data)==row['sha256'],name
  assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==blobs[name],name
  if source_root is not None:assert data==read(safe(source_root,name)),name
 compiled={name for name in source['inputs'] if name.endswith('.rs') and (re.match(r'^crates/[^/]+/(src|tests|examples|benches)/',name) or name.startswith('xtask/src/') or re.match(r'^crates/[^/]+/build\.rs$',name))}
 assert sorted(compiled)==source['workspace_rust_paths'] and len(compiled)==452
 references=[]
 for name in sorted(compiled):
  text=unpack(safe(base,source['inputs'][name]['archive'])).decode();matches=list(re.finditer(r'\b(include_str|include_bytes|include)!\s*\(\s*"([^"\\]+)"\s*\)',text))
  assert len(matches)==len(list(re.finditer(r'\b(?:include_str|include_bytes|include)!\s*\(',text))),name
  for m in matches:
   target=posixpath.normpath(str(pathlib.PurePosixPath(name).parent/m[2]));assert target in source['inputs']
   references.append({'source':name,'macro':m[1],'literal':m[2],'target':target,'sha256':source['inputs'][target]['sha256']})
 assert references==source['embedded_references'] and len(references)==61
 assert sorted({r['target'] for r in references})==source['embedded_inputs'] and len(source['embedded_inputs'])==56 and source['dynamic_includes']==0
 gate=load(base/'gate/receipt.json');assert gate['producer']==PRODUCER and gate['source_index_sha256']==sha(read(base/'source-index.json'))
 assert [r['stage'] for r in gate['rows']]==list(range(8)) and all(r['exit']==0 for r in gate['rows'])
 assert gate['jobs']==gate['test_threads']==2 and gate['profile']=='ci' and gate['per_test_timeout_unchanged'] is True
 for row in gate['rows']:assert row['log_sha256']==sha(read(base/f"gate/stage-{row['stage']}.log"))
 assert re.search(r'1326 tests run: 1326 passed(?: \(\d+ slow\))?, 0 skipped',read(base/'gate/stage-2.log').decode())
 ci=unpack(safe(base,source['inputs']['.github/workflows/ci.yml']['archive'])).decode();commands=[]
 for cmd in re.findall(r'^\s+(?:run: )?(python3[^\n]+)$',ci,re.M):
  if cmd not in commands:commands.append(cmd)
 assert commands==gate['ci_python_commands'] and len(commands)==45
 runtime=load(base/'runtime/receipt.json');assert runtime['producer']==PRODUCER and runtime['family_certified'] is False and runtime['metadata_allowlist']==ALLOWLIST
 assert (runtime['memory_selected'],runtime['disk_selected'],runtime['primary_selected'])==(232,2088,29)
 data=rows(base,'runtime',runtime);assert len(data)==2320
 cases=set(load(base/'declared-selection.json')['cases']);assert len(cases)==29 and set(runtime['refused_cases'])==REFUSED
 expected_memory=set(itertools.product(cases,['tlc','dive'],['fp','exact'],['False','True']))
 expected_disk=set(itertools.product(cases,['tlc','dive'],['fp','exact'],['1','4','8'],['None','0','7'],['False','True']))
 memory=set();disk=set()
 for name,doc in data.items():
  if name.startswith('memory-'):
   memory.add(tuple(name.removeprefix('memory-').split('-')));continue
  case,profile,store,width,chaos,budget=name.removeprefix('disk-').split('-');disk.add((case,profile,store,width,chaos,budget))
  if case in REFUSED:
   assert doc['verdict']=='unsupported' and doc['statistics'] is None and doc['output']==[] and 'seen_storage' not in doc and doc['options']['workers']['used']==0
  else:
   reference=data[f'memory-{case}-{profile}-{store}-{budget}']
   if doc['statistics'] is not None:
    seen=doc['seen_storage'];assert seen['first_failure'] is None and seen['deciding_event_before_failure'] is None
    assert seen['admitted_keys']==doc['statistics']['distinct']==seen['before_cleanup']['physical_entries']
    assert seen['before_cleanup']['resident_key_collection_bytes']==0 and seen['after_cleanup']['cleanup_completed'] is True and seen['after_cleanup']['expected_disk_bytes']==0
    workers=doc['options']['workers'];assert workers['requested']==int(width) and workers['used']==1 and workers['sequential'] is True and 'immutable disk read views' in workers['reason']
   else:assert 'seen_storage' not in doc and doc['options']['workers']['used']==0
   assert stable(doc,True,True)==stable(reference,workers=True),name
 assert memory==expected_memory and disk==expected_disk
 primary=load(base/'primary/receipt.json');assert {r['case'] for r in primary['rows']}==cases and len(primary['rows'])==29
 reference_rows={row['case']:row for row in runtime['rows'] if row['case'].startswith('memory-')}
 for row in primary['rows']:
  case=row['case'];raw=read(base/'primary'/(case+'.stdout')).decode();doc=data[f'memory-{case}-tlc-fp-False']
  assert sha(read(base/'fixture-inputs'/case/(case+'.tla')))==row['source_sha256'] and sha(read(base/'fixture-inputs'/case/(case+'.cfg')))==row['config_sha256']
  if case in REFUSED:continue
  assert reference_rows[f'memory-{case}-tlc-fp-False']['exit']==row['exit'] and doc['tlc']['verified'] is True
  assert doc['tlc']['code'] in [int(code) for code in re.findall(r'@!@!@STARTMSG (\d+):',raw)]
  if doc['statistics'] is not None:
   counts=re.search(r'(\d+) states generated, (\d+) distinct states found, (\d+) states left on queue\.',raw);depth=re.search(r'The depth of the complete state graph search is (\d+)\.',raw);assert counts and depth
   assert tuple(map(int,counts.groups()))==tuple(doc['statistics'][k] for k in ['generated','distinct','left_on_queue']) and int(depth[1])==doc['statistics']['depth']
  word=doc['counterexample'];printed=re.findall(r'@!@!@STARTMSG 2217:4 @!@!@\n(\d+): ([^\n]+)\n(.*?)@!@!@ENDMSG 2217 @!@!@',raw,re.S)
  assert (word['length'] if word else 0)==len(printed)
  if word:
   for actual,(number,label,body) in zip(word['states'],printed):assert actual['index']==int(number) and actual['label']==label and actual['text'].rstrip('\n')==body.rstrip('\n')
 api=load(base/'api-controls/stdout.json');assert api['family_certified'] is False and len(api['rows'])==2088
 labels=set();proofs=[]
 for row in api['rows']:
  name=row['label'];assert name not in labels;labels.add(name)
  assert row['cli_api_equal_except_elapsed'] is True and stable(row['report'])==stable(data[name]),name
  if row['case'] in WORDS:
   assert row['positive']=='valid' and row['mutants'] and all(m['validation']=='invalid' for m in row['mutants']);proofs.append(row)
  else:assert 'positive' not in row
 assert labels=={name for name in data if name.startswith('disk-')} and len(proofs)==432
 current=load(base/'current-source.json')
 assert set(current['current_sha256'])==set(source['inputs'])
 receiving=pathlib.Path(__file__).resolve().parent.parent
 for name,digest in current['current_sha256'].items():assert sha(read(safe(receiving,name)))==digest,name
 rust_cargo={name for name in source['inputs'] if name.endswith('.rs') or pathlib.PurePosixPath(name).name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']}
 assert current['current_rust_cargo_equals_runtime']==all(current['current_sha256'][name]==source['inputs'][name]['sha256'] for name in rust_cargo)
 assert current['current_selected_inputs_equal_runtime']==all(current['current_sha256'][name]==source['inputs'][name]['sha256'] for name in source['inputs'])
 original=load(base/'original-runtime/receipt.json');assert original['producer']==PRODUCER and original['metadata_allowlist']==ALLOWLIST
 assert (original['memory_selected'],original['disk_selected'],original['reference_selected'])==(384,576,64) and len(original['primary'])==8
 assert next(r for r in original['primary'] if r['case']=='DeterministicError')['native_error_class_verified'] is False
 original_data=rows(base,'original-runtime',original);assert len(original_data)==1024
 old={r['case']:r for r in load(base/'memory-baseline/receipt.json')['rows']}
 for row in original['rows']:
  name=row['case'];doc=original_data[name]
  if name.startswith('memory-'):
   old_name=name.removeprefix('memory-');assert row['exit']==old[old_name]['exit'];assert stable(doc)==stable(load(safe(base,'memory-baseline/'+old_name+'.stdout')))
  elif name.startswith('disk-'):
   model,profile,store,width,chaos,budget=name.removeprefix('disk-').split('-');reference=original_data[f'reference-{model}-{profile}-{store}-{budget}']
   assert doc['options']['workers']['requested']==int(width) and doc['options']['workers']['used']==1
   seen=doc['seen_storage'];assert seen['first_failure'] is None and seen['deciding_event_before_failure'] is None and seen['after_cleanup']['cleanup_completed'] is True
   assert seen['admitted_keys']==doc['statistics']['distinct']==seen['before_cleanup']['physical_entries'] and seen['before_cleanup']['resident_key_collection_bytes']==0
   assert stable(doc,True,True)==stable(reference,workers=True)
 replay=load(base/'replay-property-current/receipt.json');assert replay['producer']==PRODUCER and replay['selected']==80 and replay['certificate_credit'] is False
 replay_data=rows(base,'replay-property-current',replay);assert len(replay_data)==80
 for row in replay['rows']:
  profile,store,budget,width,chaos=row['case'].split('-');doc=replay_data[row['case']]
  assert row['exit']==(76 if profile=='tlc' else 13) and doc['verdict']=='evaluation-error'
  assert doc['tlc']['code']==2111 and doc['tlc']['exit_status']==76 and doc['tlc']['verified'] is True
  assert tuple(doc['statistics'][k] for k in ['generated','distinct','left_on_queue','depth'])==(4,4,1,3)
  assert doc['counterexample']['length']==3 and row['certificate_credit'] is False
  if width!='None':assert stable(doc,True,True)==stable(replay_data[f'{profile}-{store}-{budget}-None-None'],workers=True)
 guides=load(base/'guides-gates/receipt.json');assert guides['producer']==PRODUCER and all(r['exit']==0 for r in guides['rows'])
 for row in guides['rows']:
  for channel in ['stdout','stderr']:assert sha(read(base/'guides-gates'/(row['name']+'.'+channel)))==row[channel+'_sha256']
 for corpus,count in [('examples',21),('validation-smoke',16)]:
  summary=load(base/'guides-gates'/('gate-'+corpus+'.stdout'));assert summary['models']['agree']==count and all(v==0 for k,v in summary['models'].items() if k!='agree')
  for key in ['worker_counts_variant','worker_depth_variant','worker_property_variant','worker_verdict_variant']:assert summary[key]==0
  progress=re.findall(r'^\[\d+/\d+\] (\S+) ',read(base/'guides-gates'/('gate-'+corpus+'.stderr')).decode(),re.M);assert len(progress)==count and set(progress)=={'agree'}
  reports=list((base/'guides-gates'/('report-'+corpus)).glob('*.json'));assert len(reports)==1;report=load(reports[0]);assert len(report['models'])==count and report['settings']['workers']==[1,4,8]
  for model in report['models']:assert model['outcome']=='agree' and [r['workers'] for r in model['runs']]==[1,4,8] and all(r['outcome']=='agree' and r['diffs']==[] for r in model['runs'])
 failed=load(base/'failed-9bb/gate/receipt.json');assert failed['producer']=='9bb256a65a70513d0894ddd16d7773be68ec9d6f' and failed['rows'][-1]['exit']!=0
 assert re.search(r'1326 tests run: 1325 passed, 1 failed, 0 skipped',read(base/'failed-9bb/gate/stage-2.log').decode())
 style=load(base/'failed-526/gate/receipt.json');assert style['producer']=='526acaef1aeecb6f57a79ff05e87289e8bb9ce62' and style['rows'][-1]['exit']!=0 and len(style['rows'])==2
 records=load(base/'binaries.json')['binaries'];assert len(records)==5 and all(r['runtime_source_commit']==PRODUCER for r in records)
 if binary_root is not None:
  for record in records:assert sha(read(safe(binary_root,record['path'])))==record['sha256']
 assert runtime['binary_sha256']==next(r['sha256'] for r in records if r['name']=='dive')
 print(json.dumps({'producer':PRODUCER,'selected_source':len(source['inputs']),'literal_sites':len(references),'gate':'8/8;1326/1326;0skipped','cli_rows':len(data),'public_rows':len(api['rows']),'independent_valid_words':len(proofs),'semantic_mutants_invalid':sum(len(r['mutants']) for r in proofs),'family_certified':False}))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('packet',nargs='?',type=pathlib.Path,default=pathlib.Path(__file__).resolve().parent.parent/'conformance/probes/disk-safety-roles/implemented-epoch');p.add_argument('--source-root',type=pathlib.Path);p.add_argument('--binary-root',type=pathlib.Path);a=p.parse_args();verify(a.packet,a.source_root,a.binary_root)
