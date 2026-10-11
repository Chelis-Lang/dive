from pathlib import Path
import json,os,hashlib,subprocess,copy,itertools,re,time
root=Path('/Users/jeff/code/dive-trace-representation-work');out=Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-roles-current/runtime');binroot=out.parent/'current-binaries';primary=out.parent/'primary'
for k,v in {'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target','CARGO_BUILD_JOBS':'2','CARGO_INCREMENTAL':'0','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache'}.items():assert os.environ.get(k)==v,k
producer=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip();assert not subprocess.check_output(['git','status','--porcelain'],cwd=root)
cli=binroot/'dive';out.mkdir(parents=True,exist_ok=False);rows=[];contracts=[];start=time.monotonic();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
cases=json.loads((root/'conformance/probes/disk-safety-roles/declared-selection.json').read_text())['cases'];refused={'PrintedProperty','PrintedSpecification','AssumePrintSafety','FairSpecification','TemporalProperty','QuantifiedProperty','WrapperSpecification'};assert len(cases)==29
fixtures=root/'crates/dive-check/tests/fixtures/disk-seen-roles';refs={}
def command(name,profile,store,width,chaos,budget,inline=False):
 file=fixtures/name/(name+'.tla');args=['check',str(file),'--config',str(file.with_suffix('.cfg')),'--compat',profile,'--store',store,'--workers',str(width),'--seed','7','--fp','0','--output','json']
 if inline:args+=['--sequential']
 if chaos is not None:args+=['--chaos',str(chaos)]
 if budget:args+=['--frontier-memory-bytes','0','--predecessor-memory-bytes','0']
 return args
def run(label,args):
 cmd=[str(cli),*args];r=subprocess.run(cmd,cwd=root,capture_output=True,timeout=60)
 (out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr)
 d=json.loads(r.stdout);rows.append({'case':label,'command':cmd,'exit':r.returncode,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest(),'verdict':d['verdict']})
 (out/'running-receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'rows':rows,'elapsed_seconds':time.monotonic()-start},indent=2)+'\n');return r.returncode,d
def stable(d):
 d=copy.deepcopy(d);d.pop('elapsed_ms');return d
for name,profile,store,budget in itertools.product(cases,['tlc','dive'],['fp','exact'],[False,True]):
 label=f'memory-{name}-{profile}-{store}-{budget}';code,d=run(label,command(name,profile,store,1,None,budget,True));refs[(name,profile,store,budget)]=(code,d)
 if not budget and store=='fp':
  old=json.loads((root/'conformance/probes/disk-safety-roles/before'/(name+'.'+profile+'.memory.stdout')).read_text())
  a=stable(d);b=stable(old);a['options'].pop('workers');b['options'].pop('workers');assert a==b,label
 print(label,code,flush=True)
for name,profile,store,width,chaos,budget in itertools.product(cases,['tlc','dive'],['fp','exact'],[1,4,8],[None,0,7],[False,True]):
 label=f'disk-{name}-{profile}-{store}-{width}-{chaos}-{budget}';code,d=run(label,command(name,profile,store,width,chaos,budget)+['--seen-disk-entries','1000'])
 if name in refused:
  assert d['verdict']=='unsupported' and d['statistics'] is None and d['output']==[] and 'seen_storage' not in d and d['options']['workers']['used']==0,label
 else:
  expected_code,reference=refs[(name,profile,store,budget)];assert code==expected_code,label
  a=stable(d);b=stable(reference)
  if d['statistics'] is not None:
   facts=a.pop('seen_storage');assert facts['first_failure'] is None and facts['deciding_event_before_failure'] is None
   assert facts['admitted_keys']==d['statistics']['distinct']==facts['before_cleanup']['physical_entries']
   assert facts['before_cleanup']['resident_key_collection_bytes']==0 and facts['after_cleanup']['cleanup_completed'] is True and facts['after_cleanup']['expected_disk_bytes']==0
   w=d['options']['workers'];assert w['requested']==width and w['used']==1 and w['sequential'] is True and 'immutable disk read views' in w['reason']
  else:assert 'seen_storage' not in d and d['options']['workers']['used']==0
  a['options'].pop('seen_disk');a['options'].pop('workers');b['options'].pop('workers');assert a==b,label
 print(label,code,d['verdict'],flush=True)
# Complete original29 oracle selection remains present, including all seven refusals.
primary_rows=json.loads((primary/'receipt.json').read_text())['rows'];assert {r['case'] for r in primary_rows}==set(cases)
for row in primary_rows:
 name=row['case'];code,d=refs[(name,'tlc','fp',False)];raw=(primary/(name+'.stdout')).read_text();file=fixtures/name/(name+'.tla')
 assert sha(file)==row['source_sha256'] and sha(file.with_suffix('.cfg'))==row['config_sha256']
 if name in refused:
  contracts.append({'case':name,'outcome':'unsupported','oracle_exit':row['exit'],'resident_exit':code,'agreement_credit':False,'certificate_credit':False});continue
 assert code==row['exit'],(name,code,row['exit'])
 assert d['tlc']['verified'] and d['tlc']['code'] in [int(n) for n in re.findall(r'@!@!@STARTMSG (\d+):',raw)],name
 if d['statistics'] is not None:
  stats=re.search(r'(\d+) states generated, (\d+) distinct states found, (\d+) states left on queue\.',raw);depth=re.search(r'The depth of the complete state graph search is (\d+)\.',raw);assert stats and depth,name
  assert tuple(map(int,stats.groups()))==tuple(d['statistics'][k] for k in ['generated','distinct','left_on_queue']),name
  assert int(depth[1])==d['statistics']['depth'],name
 word=d['counterexample'];length=len(re.findall(r'@!@!@STARTMSG 2217:4',raw))
 assert (word['length'] if word else 0)==length,name
 if word:
  printed=re.findall(r'@!@!@STARTMSG 2217:4 @!@!@\n(\d+): ([^\n]+)\n(.*?)@!@!@ENDMSG 2217 @!@!@',raw,re.S)
  assert len(printed)==len(word['states']),name
  for actual,(index,label,body) in zip(word['states'],printed):
   assert actual['index']==int(index) and actual['label']==label and actual['text'].rstrip('\n')==body.rstrip('\n'),name
 contracts.append({'case':name,'outcome':'agree-supported-contract','oracle_exit':row['exit'],'code':d['tlc']['code'],'count_depth_and_trace_length_equal':True,'certificate_credit':'independent public controls separately qualified; no error certificate'})
assert len(rows)==2320 and len(contracts)==29
(out/'receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'declared_selection_sha256':sha(root/'conformance/probes/disk-safety-roles/declared-selection.json'),'memory_selected':232,'disk_selected':2088,'primary_selected':29,'rows':rows,'contracts':contracts,'refused_cases':sorted(refused),'metadata_allowlist':['elapsed_ms','options.workers','options.seen_disk','seen_storage'],'elapsed_seconds':time.monotonic()-start,'family_certified':False},indent=2)+'\n')
print('CLOSED232memory2088disk29primarycontracts',flush=True)
