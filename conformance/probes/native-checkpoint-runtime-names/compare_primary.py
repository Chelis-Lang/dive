"""Compare five native TLC-mode concrete traces with clean pinned primary output.

Run from the repository root with DIVE and a fresh output directory. The parser
handles only these bounded scalar/model-value fixtures, not arbitrary TLA values.
This comparison is not an independent runtime-history validator certificate.
"""
from pathlib import Path
import hashlib,json,re,subprocess,sys
root=Path.cwd();b=root/'conformance/probes/native-checkpoint-runtime-names';out=Path(sys.argv[2]).resolve();out.mkdir(exist_ok=False,parents=True)
binary=Path(sys.argv[1]).resolve();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();idx={'native_binary_sha256':sha(binary),'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'capture_script_sha256':sha(Path(__file__)),'scope':'direct clean primary counts and concrete trace comparison, not independent validator certification','controls':[]}
for d in sorted((b/'specs').iterdir()):
 case=d.name;primary=b/'tlc-primary-final'/(case+'-seed7.stdout');text=primary.read_text();states=[]
 for block in re.findall(r'@!@!@STARTMSG 2217:4 @!@!@\n(.*?)@!@!@ENDMSG 2217',text,re.S):
  state={}
  for key,value in re.findall(r'^/\\ (\w+) = (.+)$',block,re.M):
   state[key]=json.loads(value) if value.startswith('"') or value.isdigit() else {'model':value}
  assert state;states.append(state)
 args=['check',str((d/(case+'.tla')).relative_to(root)),'--compat=tlc','--seed','7','--fp','0','--workers','1','--output','json']
 r=subprocess.run([str(binary),*args],capture_output=True,timeout=45)
 for stream in ['stdout','stderr']:(out/(case+'.'+stream)).write_bytes(getattr(r,stream))
 v=json.loads(r.stdout);native=[s['values'] for s in v['counterexample']['trace']['states']]
 assert r.returncode==12 and v['verdict']=='invariant-violation'
 assert states==native,(case,states,native)
 counts=list(map(int,re.search(r'(\d+) states generated, (\d+) distinct states found, (\d+) states left on queue',text).groups()))
 assert counts==[v['statistics'][key] for key in ['generated','distinct','left_on_queue']]
 assert len(states)==v['statistics']['depth']==4
 idx['controls'].append({'case':case,'args':args,'exit':r.returncode,'primary_stdout':str(primary.relative_to(root)),'primary_sha256':sha(primary),'counts':counts,'concrete_states_equal':True,'stdout':case+'.stdout','stdout_sha256':sha(out/(case+'.stdout')),'stderr':case+'.stderr','stderr_sha256':sha(out/(case+'.stderr'))})
 print(case,counts,'concrete trace equals clean primary')
(out/'index.json').write_text(json.dumps(idx,indent=2)+'\n')
