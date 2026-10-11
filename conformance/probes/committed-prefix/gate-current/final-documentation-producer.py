from pathlib import Path
import subprocess,json,hashlib,gzip,os,time
r=Path.cwd();b=r/'conformance/probes/committed-prefix/gate-current'; sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
old=json.loads((b/'before.json').read_text());closed=json.loads((b/'after.json').read_text());assert closed['all_steps_passed'] and closed['build_inputs_unchanged'];assert old['producer']=='ed31d97378d43744c89a2bac2a48307ca10251ac'
head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();assert head!=old['producer'];assert not subprocess.check_output(['git','status','--porcelain=v1','--untracked-files=no'],text=True).strip()
inputs={p:sha(r/p)for p in old['source_sha256']};changed=[p for p,d in inputs.items()if d!=old['source_sha256'][p]];assert changed==['crates/dive-check/src/capture.rs'],changed
original=gzip.decompress((b/'source'/ (changed[0]+'.gz')).read_bytes());final=(r/changed[0]).read_bytes()
strip=lambda x:b'\n'.join(l for l in x.splitlines()if not l.lstrip().startswith(b'///'))
assert strip(original)==strip(final)
for p,h in inputs.items():
 assert hashlib.sha256(subprocess.check_output(['git','show',head+':'+p])).hexdigest()==h
 dest=b/'runtime-source'/(p+'.gz');dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(gzip.compress((r/p).read_bytes(),mtime=0))
(b/'runtime-before.json').write_text(json.dumps({'producer':head,'source_sha256':inputs,'full_gate_producer':old['producer'],'qualification':'Only capture.rs /// captions changed after the immutable full gate. Byte-for-byte non-doc lines are identical; new docs/source checks and all runtime consumers identify this separate producer.'},indent=2)+'\n')
s=(r/'AGENTS.md').read_text();start=s.index('cargo fmt --all --check');end=s.index('```',start);full=[l.split('#')[0].strip()for l in s[start:end].splitlines()if l.strip()]; commands=[l for l in full if l.startswith(('cargo fmt','python3','cargo clippy','cargo test --workspace --doc','RUSTDOCFLAGS='))];commands.insert(-2,'cargo nextest run --locked -p dive-liveness --test periodic_snapshot -E \'test(completed_prefix_graph_)\' --test-threads 2 --profile ci')
env=os.environ.copy();env.update(CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_TARGET_DIR='/Users/jeff/code/dive-periodic-snapshot-target',DIVE_CACHE_DIR='/Users/jeff/code/dive-periodic-snapshot-cache',DIVE_PERIODIC_SNAPSHOT_CAPTURE=str(r/'conformance/probes/committed-prefix/native-docs-snapshot'),DIVE_PROGRESS_QUEUE_NATIVE_CAPTURE=str(r/'conformance/probes/committed-prefix/native-docs'))
steps=[]
with(b/'final-documentation-checks.log').open('w')as log:
 for cmd in commands:
  t=time.monotonic();print(cmd,flush=True);p=subprocess.run(cmd,cwd=r,env=env,shell=True,stdout=log,stderr=subprocess.STDOUT);steps.append({'command':cmd,'exit':p.returncode,'seconds':time.monotonic()-t});(b/'final-documentation-steps.json').write_text(json.dumps(steps,indent=2)+'\n');assert p.returncode==0,cmd
unchanged=all(sha(r/p)==h for p,h in inputs.items());assert unchanged
(b/'final-documentation-closed.json').write_text(json.dumps({'production_commit':head,'full_gate_producer':old['producer'],'changed_source':changed,'only_doc_lines_changed':True,'build_inputs_unchanged':unchanged,'source_sha256':inputs,'steps':steps,'scope':'Separate final documentation/source-map qualification and unchanged eight focused prefix controls; full workspace test gate remains attributed to the earlier committed producer.'},indent=2)+'\n');print('Closed final documentation qualification',len(steps),'stages',head)
