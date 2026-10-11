from pathlib import Path
import subprocess,json,hashlib,re,gzip,os
import argparse
p=argparse.ArgumentParser();p.add_argument('--worktree',type=Path,required=True);p.add_argument('--artifacts',type=Path,required=True);p.add_argument('--binary',type=Path,required=True);p.add_argument('--context',type=Path,required=True);p.add_argument('--target',type=Path,required=True);p.add_argument('--out',type=Path,required=True);args=p.parse_args()
W=args.worktree;A=args.artifacts;O=args.out;O.mkdir(parents=True,exist_ok=False)

def sha(b):return hashlib.sha256(b).hexdigest()
rows=[]
for syntax,body in [('P','(* --algorithm A variables g=0;\nprocess P \\in {1,2} begin L:g:=1; E:skip; end process;\nend algorithm; *)'),('C','(* --algorithm A { variables g=0;\nprocess (P \\in {1,2}) { L:g:=1; E:skip; } } *)')]:
 source='---- MODULE SourceOptions ----\nEXTENDS Integers, TLC, Sequences\n\\* PlusCal options (-wf)\n'+body+'\n====\n'
 d=O/syntax;d.mkdir(exist_ok=True);f=d/'SourceOptions.tla';f.write_text(source)
 for profile in ['dive','tlc']:
  for mode in ['translate','rewrite']:
   cmd=[str(args.binary),'translate',str(f),'-sf','--output','json']+(['--rewrite']if mode=='rewrite'else[])+(['--compat=tlc']if profile=='tlc'else[])
   p=subprocess.run(cmd,capture_output=True,timeout=30);doc=json.loads(p.stdout);assert p.returncode==0 and doc['accepted'] and doc['model_proved']is False and doc['option_context']['requested']['fairness']=='Strong' and doc['option_context']['effective']['fairness']=='Weak'
   label=f'{syntax}-{profile}-{mode}';(O/(label+'.json')).write_bytes(p.stdout);rows.append({'id':label,'outcome':'accepted-context-proved','source_sha256':sha(f.read_bytes()),'command':cmd,'binary_sha256':sha(Path(cmd[0]).read_bytes()),'stdout_sha256':sha(p.stdout),'model_proved':False})
  p=subprocess.run([str(args.context),str(f),'rewrite','strong',profile],capture_output=True,timeout=30);doc=json.loads(p.stdout);assert p.returncode==0 and doc['accepted'] and doc['model_proved']is False and doc['option_context']['effective']['fairness']=='Weak'
  label=f'{syntax}-{profile}-cargo-example';(O/(label+'.json')).write_bytes(p.stdout);rows.append({'id':label,'outcome':'accepted-context-proved','source_sha256':sha(f.read_bytes()),'binary_sha256':sha((args.context).read_bytes()),'stdout_sha256':sha(p.stdout),'model_proved':False})
project=A/'guide-cargo';(project/'src').mkdir(parents=True,exist_ok=True)
(project/'Cargo.toml').write_text('[package]\nname="source-options-guide"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\ndive-api={path="'+str(W/'crates/dive-api')+'"}\ndive-pluscal={path="'+str(W/'crates/dive-pluscal')+'"}\n')
code=re.findall(r'```rust\n(.*?)\n```',(W/'docs/pluscal-source-options.md').read_text(),re.S)[0]
(project/'src/main.rs').write_text('fn main() {\n'+code+'\nprintln!("source-options-guide: accepted, effective=Weak, model_proved=false");\n}\n')
env={**os.environ,'CARGO_BUILD_JOBS':'2','CARGO_INCREMENTAL':'0','CARGO_TARGET_DIR':str(args.target)}
p=subprocess.run(['cargo','run','--offline','--manifest-path',str(project/'Cargo.toml')],cwd=W,env=env,capture_output=True,timeout=300)
(O/'rust.stdout').write_bytes(p.stdout);(O/'rust.stderr').write_bytes(p.stderr);assert p.returncode==0,p.stderr.decode()
rows.append({'id':'rust-doc-exact','outcome':'compiled-and-executed','source_sha256':sha((project/'src/main.rs').read_bytes()),'lock_sha256':sha((project/'Cargo.lock').read_bytes()),'stdout_sha256':sha(p.stdout),'model_proved':False})
report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'selected':len(rows),'family_certified':False,'model_proved':False,'rows':rows};(O/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Guide controls passed:',len(rows),flush=True)
