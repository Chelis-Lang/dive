from pathlib import Path
import subprocess,json,hashlib,copy,itertools,time,tempfile,os
REQ={'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target','CARGO_BUILD_JOBS':'2','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache'}
for k,v in REQ.items():assert os.environ.get(k)==v,(k,os.environ.get(k))
ROOT=Path('/Users/jeff/code/dive-trace-representation-work');ART=Path('/Users/jeff/code/dive-storage-tuple-function-artifacts');OUT=ART/'current-runtime';BIN=ART/'current-binaries/dive'
OUT.mkdir(exist_ok=False);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();producer=json.loads((BIN.parent/'producer.json').read_text());assert sha(BIN)==producer['binaries_sha256']['dive']
assert producer['source_commit']==subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
rows=[];validation=[];baselines={};started=time.monotonic()
allow={'within_store':['elapsed_ms','checkpoint','frontier_storage','predecessor_storage','options.checkpoint','options.profile','options.workers','options.frontier_memory_bytes_per_queue','options.frontier_codec_nodes_per_value_limit','options.frontier_record_bytes_limit','options.predecessor_memory_bytes','options.predecessor_path_steps_limit'],'cross_store_extra':['options.store','options.fingerprint']}
def run(cmd,p,name,doc=True):
 r=subprocess.run(cmd,cwd=ROOT,capture_output=True,timeout=30);(p/(name+'.stdout')).write_bytes(r.stdout);(p/(name+'.stderr')).write_bytes(r.stderr)
 return r.returncode,json.loads(r.stdout) if doc else None

def obs(j,cross=False):
 d=copy.deepcopy(j)
 for k in allow['within_store']:
  if '.'in k:a,b=k.split('.');d[a].pop(b,None)
  else:d.pop(k,None)
 if cross:
  for k in allow['cross_store_extra']:a,b=k.split('.');d[a].pop(b)
 return d
primary=json.loads((ROOT/'conformance/probes/storage-tuple-function/before/manifest.json').read_text())['primary'];jar=Path(primary['command'][2]);java=primary['command'][0];assert sha(jar)==primary['jar_sha256'];oracles=[]
for name in ['Apply','Witness']:
 folder=OUT/('primary-'+name);folder.mkdir();source=ROOT/'conformance/probes/storage-tuple-function/specs'/name/'TupleStorage.tla'
 with tempfile.TemporaryDirectory(prefix='dive-tuple195-oracle-') as meta:
  cmd=[java,'-cp',str(jar),'tlc2.TLC','-tool','-workers','1','-seed','7','-fp','0','-cleanup','-metadir',meta,'TupleStorage'];r=subprocess.run(cmd,cwd=source.parent,capture_output=True,timeout=30)
 (folder/'stdout').write_bytes(r.stdout);(folder/'stderr').write_bytes(r.stderr)
 assert r.returncode==(75 if name=='Apply'else 12),name
 text=r.stdout.decode();assert ('1 states generated, 1 distinct states found' if name=='Apply' else '4 states generated, 4 distinct states found')in text
 if name=='Apply':assert '@!@!@STARTMSG 1000:1' in text and '@!@!@STARTMSG 2103:1' in text and 'Attempted to apply function with integer domain to the non-integer argument "a"'in text
 oracles.append({'name':name,'command':cmd,'exit':r.returncode,'jar_sha256':sha(jar),'spec_sha256':sha(source),'config_sha256':sha(source.with_suffix('.cfg'))})
for name,compat,store,width,chaos in itertools.product(['Apply','Witness'],[True,False],['fp','exact'],[1,4,8],[None,13]):
 profile='tlc'if compat else'dive';source=ROOT/'conformance/probes/storage-tuple-function/specs'/name/'TupleStorage.tla';ident=f'{name}-{profile}-{store}-w{width}-c{chaos}';folder=OUT/ident;folder.mkdir();common=[str(BIN),'check',str(source),'--seed','7','--fp','0','--workers',str(width),'--store',store,'--trace-format','representation-v2','--output','json']+(['--compat=tlc']if compat else[])+(['--chaos',str(chaos)]if chaos is not None else[])
 for mode,flags in [('memory',[]),('frontier',['--frontier-memory-bytes','0']),('predecessor',['--predecessor-memory-bytes','0']),('both',['--frontier-memory-bytes','0','--predecessor-memory-bytes','0'])]:
  p=folder/mode;p.mkdir();status,j=run(common+flags,p,'check');expected=(75 if compat else 13)if name=='Apply'else(12 if compat else 10);assert status==expected,(ident,mode,status,j)
  assert j['verdict']==('evaluation-error'if name=='Apply'else'invariant-violation');assert tuple(j['statistics'][k]for k in['generated','distinct','depth'])==((1,1,1)if name=='Apply'else(4,4,4))
  if name=='Apply':assert j['message'].endswith('Attempted to apply function with integer domain to the non-integer argument "a"')and j['tlc']['reported']==1000 and j['tlc']['code']==2103 and j['tlc']['verified']is False
  assert j['options']['workers']['requested']==j['options']['workers']['used']==width
  assert j['options']['store']==('fingerprint'if store=='fp'else'exact');assert j['options']['fingerprint']==({'polynomial_index':0,'polynomial_source':'flag','scheme':'tlc'if compat else'dive'}if store=='fp'else None);assert j['collision']is None
  cp=p/'current.cp';ps,pj=run(common+flags+['--checkpoint',str(cp),'--checkpoint-after-levels','0','--checkpoint-counters','retain'],p,'pause');assert pj['verdict']=='resource-limit'and pj['checkpoint']['status']=='paused-incomplete'and pj['options']['workers']['used']==1 and pj['counterexample']is None
  raw=cp.read_bytes();assert raw[:8]==b'DIVECP02';payload=json.loads(raw[48:]);assert payload['identity']['schema']==2 and payload['identity']['semantics'].endswith('0060-structural-v2')
  rs,rj=run(common+flags+['--recover',str(cp),'--checkpoint-counters','retain'],p,'recover');assert rs==expected and rj['checkpoint']['status']=='recovered'and rj['checkpoint']['transient_storage_cleanup_completed']is True and rj['options']['checkpoint']['independent_certificate']is False
  assert rj['options']['workers']['used']==width and rj['options']['profile']==('tlc+overrides'if compat else'dive')
  for doc in[j,rj]:
   for key,cross in[((name,profile,store),False),((name,profile,'cross'),True)]:
    value=obs(doc,cross)
    if key in baselines:assert value==baselines[key],(ident,mode,key)
    else:baselines[key]=value
  if name=='Witness':
   vcmd=[str(BIN),'validate-trace',str(source),'--seed','7','--fp','0','--output','json']+(['--compat=tlc']if compat else[])
   for stage,doc in [('check',j),('recover',rj)]:
    word=doc['counterexample']['trace'];assert word['states'][0]['values']['s']['repr']=='tuple-fcn'
    for kind,at in [('original',None),('mutant-init',0),('mutant-middle',1),('mutant-final',3)]:
     w=copy.deepcopy(word)
     if at is not None:w['states'][at]['values']['n']=99
     path=p/(stage+'-'+kind+'.json');path.write_text(json.dumps(w)+'\n');vs,v=run(vcmd+['--trace',str(path)],p,stage+'-'+kind);assert vs==(0 if at is None else 1)and v['result']==('valid'if at is None else'invalid'),(ident,mode,kind,v)
     validation.append({'case':ident,'mode':mode,'stage':stage,'kind':kind,'exit':vs,'result':v['result']})
  rows.append({'case':ident,'mode':mode,'check_exit':status,'pause_exit':ps,'recover_exit':rs,'files_sha256':{str(q.relative_to(p)):sha(q)for q in p.iterdir()if q.is_file()}})
 print('closed',ident,flush=True)
old=[]
for compat,store,width in itertools.product([True,False],['fp','exact'],[1,4,8]):
 cp=ROOT/f'conformance/probes/storage-tuple-function/before/{"tlc"if compat else"dive"}-{store}-w{width}.cp';assert cp.read_bytes()[:8]==b'DIVECP01';p=OUT/f'old-{compat}-{store}-{width}';p.mkdir();cmd=[str(BIN),'check',str(ROOT/'conformance/probes/storage-tuple-function/specs/Apply/TupleStorage.tla'),'--recover',str(cp),'--seed','7','--fp','0','--workers',str(width),'--store',store,'--output','json']+(['--compat=tlc']if compat else[]);s,_=run(cmd,p,'refusal',False);assert s!=0 and 'DIVE-S0400: native magic/schema mismatch'in(p/'refusal.stderr').read_text() and not(p/'refusal.stdout').read_bytes();old.append({'profile':compat,'store':store,'workers':width,'exit':s,'archive_sha256':sha(cp)})
assert len(rows)==192 and len(validation)==768 and len(old)==12
receipt={'runtime_producer':producer,'rows':rows,'validation':validation,'old_archives':old,'primary':oracles,'precise_equality_allowlist':allow,'scope':'Native exact function diagnostics EC1000/2103 retain verified:false;192check/pause/recovery triples,96Valid/288Invalid per stage; old actual archives refuse. Trusted-local archive admission is not reachability/completeness/authentication proof.','seconds':time.monotonic()-started,'driver_sha256':sha(Path(__file__))}
(OUT/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print('CLOSED192 triples/192Valid576Invalid/12oldrefusals',flush=True)
