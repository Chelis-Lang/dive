from pathlib import Path
import subprocess,json,hashlib,copy,itertools,time
ROOT=Path('/Users/jeff/code/dive-trace-representation-work')
OUT=Path('/Users/jeff/code/dive-trace-representation-artifacts/current-runtime')
BIN=Path('/Users/jeff/code/dive-trace-representation-artifacts/current-binaries/dive')
OUT.mkdir(parents=True,exist_ok=False)
sha=lambda b:hashlib.sha256(b).hexdigest()
producer=json.loads((BIN.parent/'producer.json').read_text());assert sha(BIN.read_bytes())==producer['binaries_sha256']['dive']
rows=[];validation=[];baselines={};started=time.monotonic()
def run(cmd,folder,name):
 r=subprocess.run(cmd,cwd=ROOT,capture_output=True,timeout=30)
 (folder/(name+'.stdout')).write_bytes(r.stdout);(folder/(name+'.stderr')).write_bytes(r.stderr)
 return r.returncode,json.loads(r.stdout)
def obs(j,transport=False,cross=False):
 d=copy.deepcopy(j);d.pop('elapsed_ms');d['options'].pop('workers')
 if cross:d['options'].pop('store');d['options'].pop('fingerprint')
 if transport:
  d['counterexample'].pop('trace');d['counterexample'].pop('trace_encoding',None);d['options'].pop('trace_format',None)
 return d
for name,compat,store,width,chaos in itertools.product(['Mixed','TupleFunction','RecordFunction'],[True,False],['fp','exact'],[1,4,8],[None,19]):
 profile='tlc'if compat else'dive';ident=f'{name}-{profile}-{store}-w{width}-c{chaos}';folder=OUT/ident;folder.mkdir()
 source=ROOT/'conformance/probes/trace-representation-v2/specs'/name/'Transport.tla'
 common=[str(BIN),'check',str(source),'--seed','7','--fp','0','--workers',str(width),'--store',store,'--output','json']+(['--compat=tlc']if compat else[])+(['--chaos',str(chaos)]if chaos is not None else[])
 status,j=run(common+['--trace-format','representation-v2'],folder,'v2')
 assert status==(12 if compat else 10)and j['verdict']=='invariant-violation'and j['property']=='Inv',(ident,j)
 assert tuple(j['statistics'][k]for k in['generated','distinct','depth'])==(4,4,4),ident
 assert j['options']['workers']['used']==width and j['options']['workers']['requested']==width
 assert j['options']['profile']==profile and j['options']['workers']['chaos']==chaos
 assert j['options']['fingerprint']==({'polynomial_index':0,'polynomial_source':'flag','scheme':'tlc'if compat else'dive'}if store=='fp'else None)
 assert j['collision']is None and j['output']==[]
 assert j['counterexample']['length']==4 and j['counterexample']['trace']['version']==2
 assert j['counterexample']['trace_encoding']['status']=='encoded' and j['counterexample']['trace_encoding']['semantic_validation']is False
 within=(name,profile,store);value=obs(j)
 if within in baselines:assert value==baselines[within],ident
 else:baselines[within]=value
 cross=(name,profile,'cross-store');value=obs(j,cross=True)
 if cross in baselines:assert value==baselines[cross],ident
 else:baselines[cross]=value
 legacy_status,legacy=run(common+['--trace-format','v1'],folder,'v1')
 assert legacy_status==status and legacy['counterexample']['trace']['version']==1 and 'trace_encoding'not in legacy['counterexample']
 assert obs(legacy,transport=True)==obs(j,transport=True),ident
 vcmd=[str(BIN),'validate-trace',str(source),'--seed','7','--fp','0','--output','json']+(['--compat=tlc']if compat else[])
 for kind,at in [('original',None),('mutant-init',0),('mutant-middle',1),('mutant-final',3)]:
  word=copy.deepcopy(j['counterexample']['trace'])
  if at is not None:word['states'][at]['values']['n']=99
  wordpath=folder/(kind+'.json');wordpath.write_text(json.dumps(word)+'\n')
  vs,v=run(vcmd+['--trace',str(wordpath)],folder,kind)
  assert vs==(0 if at is None else 1)and v['result']==('valid'if at is None else'invalid'),(ident,kind,v)
  validation.append({'case':ident,'kind':kind,'exit':vs,'result':v['result']})
 if name=='Mixed':
  ls,lv=run(vcmd+['--trace',str(folder/'v1.stdout')],folder,'legacy-shape')
  assert ls==1 and lv['result']=='invalid'and lv['failure']['check']=='shape'
 rows.append({'case':ident,'profile':profile,'store':store,'workers':width,'chaos':chaos,'exit':status,'legacy_exit':legacy_status,'files_sha256':{p.name:sha(p.read_bytes())for p in folder.iterdir()if p.is_file()}})
 print(len(rows),ident,flush=True)
assert len(rows)==72 and len(validation)==288
receipt={'runtime_producer':producer,'rows':rows,'validation':validation,'precise_equality_allowlist':{'within_store':['elapsed_ms','options.workers'],'cross_store_extra':['options.store','options.fingerprint'],'transport_only_extra':['counterexample.trace','counterexample.trace_encoding','options.trace_format']},'legacy_scope':'72 matching checks;24 Mixed decoder-shape refusals remain separate from v2 replay','primary_scope':'3 clean pinned verdict/count/property/depth controls; stored flags/fingerprints are separately tested in compiled API/unit controls','driver_sha256':sha(Path(__file__).read_bytes()),'total_seconds':time.monotonic()-started}
(OUT/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print('CLOSED72 v2+72v1 /72Valid216Invalid',flush=True)
