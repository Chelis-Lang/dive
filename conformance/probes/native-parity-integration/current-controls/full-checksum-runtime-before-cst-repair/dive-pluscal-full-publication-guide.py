from pathlib import Path
import subprocess,json,hashlib,re,os,sys
root,binary,target,out=map(lambda p:Path(p).resolve(),sys.argv[1:]);out.mkdir(parents=True,exist_ok=False)
source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip();assert source=='d7c4f5911a1e68d21a10f67bb3bee0546d6448c6'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();guide=(root/'docs/pluscal-rewriting.md').read_text()
tla=re.findall(r'```tla\n(.*?)\n```',guide,re.S);rust=re.findall(r'```rust\n(.*?)\n```',guide,re.S);assert len(tla)==2 and len(rust)==1
sources=[tla[0]+'\n',tla[0].split('(*')[0]+tla[1]+'\n\\* Keep this comment.\n====\n'];rows=[]
env=os.environ.copy()
for k in ['DIVE_COMPAT','DIVE_CHAOS']:env.pop(k,None)
env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0')
for syntax,text in zip(['P','C'],sources):
 work=out/syntax;work.mkdir();file=work/'Rewrite.tla';file.write_text(text)
 for profile in ['dive','tlc']:
  for policy in ['full','tla']:
   args=['translate',file.name,'--rewrite','--checksums',policy,'--output','json']+(['--compat=tlc']if profile=='tlc'else[])
   r=subprocess.run([str(binary),*args],cwd=work,env=env,capture_output=True,timeout=30);label=profile+'-'+policy
   (work/(label+'.stdout')).write_bytes(r.stdout);(work/(label+'.stderr')).write_bytes(r.stderr)
   d=json.loads(r.stdout);assert r.returncode==0 and d['accepted'] and d['model_proved'] is False and d['exhaustive'] is False and file.read_text()==text
   assert d['profile']==('tlc+overrides'if profile=='tlc'and policy=='tla'else profile)
   assert d['algorithm_checksum_verified']==(policy=='full')
   rows.append({'syntax':syntax,'args':args,'exit':r.returncode,'source_sha256':sha(file),'stdout_sha256':sha(work/(label+'.stdout')),'stderr_sha256':sha(work/(label+'.stderr'))})
 for kind in ['write','multiline']:
  altered=text if kind=='write'else text.replace('x = 0','x = 0 +\n1');file.write_text(altered)
  args=['translate',file.name,'--rewrite']+(['--write']if kind=='write'else[])+['--output','json']
  r=subprocess.run([str(binary),*args],cwd=work,env=env,capture_output=True,timeout=30);(work/(kind+'.stdout')).write_bytes(r.stdout);(work/(kind+'.stderr')).write_bytes(r.stderr)
  assert r.returncode==3 and file.read_text()==altered
  rows.append({'syntax':syntax,'control':kind,'args':args,'exit':r.returncode,'stdout_sha256':sha(work/(kind+'.stdout')),'stderr_sha256':sha(work/(kind+'.stderr'))})
caller=out/'caller';(caller/'src').mkdir(parents=True)
(caller/'Cargo.toml').write_text('[package]\nname="full-rewrite-publication-guide"\nversion="0.0.0"\nedition="2024"\npublish=false\n[dependencies]\ndive-api={path='+json.dumps(str(root/'crates/dive-api'))+'}\n')
(caller/'src/main.rs').write_text('fn main() {\n'+rust[0]+'\nprintln!("{}", report.to_json());\n}\n')
r=subprocess.run(['cargo','run','--offline','--manifest-path',str(caller/'Cargo.toml')],cwd=out,env=env,capture_output=True,timeout=600);(out/'rust.stdout').write_bytes(r.stdout);(out/'rust.stderr').write_bytes(r.stderr)
assert r.returncode==0,r.stderr.decode()[-4000:];d=json.loads(r.stdout);assert d['accepted'] and d['model_proved'] is False and d['body_checksum_verified'] is True and d['algorithm_checksum_verified'] is True
receipt={'schema_version':1,'source_commit':source,'guide_sha256':sha(root/'docs/pluscal-rewriting.md'),'binary_sha256':sha(binary),'controls':rows,'exact_cargo_owned_rust_guide':True,'family_certified':False}
receipt['evidence_sha256']={str(p.relative_to(out)):sha(p)for p in out.rglob('*')if p.is_file()};(out/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n');print('12 fresh CLI controls and exact Cargo-owned Full checksum guide passed')
