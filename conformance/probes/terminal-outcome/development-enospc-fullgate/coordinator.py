from pathlib import Path
import subprocess,json,hashlib,time,os,gzip,re
r=Path.cwd();b=Path('/tmp/dive-terminal-classification-fullgate');b.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
tracked=subprocess.check_output(['git','ls-files'],text=True).splitlines()
inputs={p:sha(r/p)for p in tracked if (p.endswith('.rs') or p.endswith('.toml') or p in ['Cargo.lock'])and not p.startswith('conformance/')}
# Literal embedded Rust inputs are actual compiler inputs, not just evidence files.
include_pattern=re.compile(r'\binclude(?:_str|_bytes)?!\s*\(\s*"([^"\\]*(?:\\.[^"\\]*)*)"\s*,?\s*\)')
include_sources={}; unresolved=[]
queue=[p for p in inputs if p.endswith('.rs')]; scanned=set()
while queue:
 source=queue.pop()
 if source in scanned:continue
 scanned.add(source);text=(r/source).read_text()
 literals=list(include_pattern.finditer(text))
 calls=list(re.finditer(r'\binclude(?:_str|_bytes)?!\s*\(',text))
 if len(literals)!=len(calls):unresolved.append(source)
 for match in literals:
  literal=json.loads('"'+match.group(1)+'"')
  path=((r/source).parent/literal).resolve()
  relative=path.relative_to(r.resolve()).as_posix()
  assert path.is_file(),relative
  inputs[relative]=sha(path)
  include_sources.setdefault(relative,[]).append(source)
  if relative.endswith('.rs'):queue.append(relative)
assert not unresolved,unresolved
inputs=dict(sorted(inputs.items()))
head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
assert not subprocess.check_output(['git','status','--porcelain=v1','--untracked-files=no'],text=True).strip()
for p,d in inputs.items():
 out=b/'source'/(p+'.gz');out.parent.mkdir(parents=True,exist_ok=True)
 with out.open('wb')as f:
  with gzip.GzipFile(fileobj=f,mode='wb',mtime=0)as g:g.write((r/p).read_bytes())
(b/'before.json').write_text(json.dumps(dict(producer=head,scope='Clean committed terminal graph/postcondition composition Rust/Cargo plus literal embedded include inputs; actual gate outcome, no ordinary schedule/model proof certification',source_sha256=inputs,literal_include_sources=include_sources,unresolved_include_macros=unresolved),indent=2)+'\n')
s=Path('AGENTS.md').read_text();start=s.index('cargo fmt --all --check');end=s.index('```',start);commands=[l.split('#')[0].strip()for l in s[start:end].splitlines()if l.strip()]
commands=[l.replace('cargo nextest run --workspace --locked','cargo nextest run --workspace --locked --test-threads 2')for l in commands]
env=os.environ.copy();env.update(CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR='/Users/jeff/code/dive-periodic-snapshot-target',DIVE_CACHE_DIR='/Users/jeff/code/dive-periodic-snapshot-cache',DIVE_PERIODIC_SNAPSHOT_CAPTURE='/tmp/dive-terminal-classification-legacy-snapshot',DIVE_PROGRESS_QUEUE_NATIVE_CAPTURE='/tmp/dive-terminal-classification-legacy-request',DIVE_REQUEST_SERVICE_CAPTURE='/tmp/dive-terminal-classification-legacy-service',DIVE_OPTIONAL_PERIODIC_BINDING_CAPTURE='/tmp/dive-terminal-classification-binding-current.json')
steps=[]
with(b/'workspace.log').open('w')as log:
 for command in commands:
  print(command,flush=True);log.write('\nCOMMAND: '+command+'\n');log.flush();start=time.monotonic();result=subprocess.run(command,shell=True,cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT)
  steps.append(dict(command=command,exit=result.returncode,wall_seconds=time.monotonic()-start));(b/'steps.json').write_text(json.dumps(steps,indent=2)+'\n')
  if result.returncode:print('FAILED',command,flush=True);break
unchanged=all(sha(r/p)==d for p,d in inputs.items());passed=len(steps)==len(commands)and all(x['exit']==0 for x in steps)
(b/'after.json').write_text(json.dumps(dict(producer=head,build_inputs_unchanged=unchanged,source_sha256=inputs,all_steps_passed=passed,expected_steps=len(commands),log_sha256=sha(b/'workspace.log')),indent=2)+'\n')
assert unchanged
print('Closed gate',len(steps),'of',len(commands),'passed=',passed,flush=True)
raise SystemExit(0 if passed else 1)
