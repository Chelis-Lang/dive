from pathlib import Path
import subprocess,os,json,hashlib,time
W=Path('/Users/jeff/code/dive-pluscal-definition-forms-work');O=Path('/Users/jeff/code/dive-pluscal-process-procedure-checksum-artifacts/ownership-budget-mutations');O.mkdir(exist_ok=False)
files=[W/'crates/dive-pluscal/src/translate/checksum.rs',W/'crates/dive-pluscal/src/translate/checksum_transfers.rs'];original=[p.read_bytes()for p in files]
sha=lambda b:hashlib.sha256(b).hexdigest()
for p,data in zip(files,original):(O/(p.name+'.original')).write_bytes(data)
env=os.environ.copy();env.update(CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR='/Users/jeff/code/dive-pluscal-macro-checksum-target');rows=[]
try:
 for kind in ['omit-process-called','leak-process-into-LAST','disable-shared-work-charge','restored']:
  for p,data in zip(files,original):p.write_bytes(data)
  s=files[0].read_text();t=files[1].read_text()
  if kind=='omit-process-called':
   before='let count = self.header.process_called[owner].len();';assert before in s;s=s.replace(before,'let count = 0;',1)
  elif kind=='leak-process-into-LAST':
   before='let last = matches!(parsed.algorithm.body, Body::Sequential(_))\n            && parsed';assert before in s;s=s.replace(before,'let last = parsed',1)
   before='builder.collect_calls()?;';assert before in t;t=t.replace(before,before+'\n    if let Some(called) = builder.plan.process_called.first() { builder.plan.main_called = called.clone(); }',1)
  elif kind=='disable-shared-work-charge':
   before='self.work > self.work_limit';assert before in t;t=t.replace(before,'false',1)
  files[0].write_text(s);files[1].write_text(t)
  for p in files:(O/(kind+'.'+p.name)).write_bytes(p.read_bytes())
  command=['cargo','test','--locked','-p','dive-pluscal','--test','process_procedure_checksum']
  if kind=='disable-shared-work-charge':command+=['process_calls_share_the_existing_expansion_and_output_bounds']
  elif kind!='restored':command+=['process_original_header_bytes_and_crc_match_every_admitted_row']
  command+=['--','--test-threads=2'];q=subprocess.run(command,cwd=W,env=env,capture_output=True,timeout=180)
  (O/(kind+'.stdout')).write_bytes(q.stdout);(O/(kind+'.stderr')).write_bytes(q.stderr)
  assert q.returncode==(0 if kind=='restored'else101),(kind,q.returncode,q.stdout,q.stderr)
  assert kind=='restored'or b'test result: FAILED'in q.stdout
  rows.append({'mutation':kind,'command':command,'exit_code':q.returncode,'source_sha256':{p.name:sha(p.read_bytes())for p in files},'stdout_sha256':sha(q.stdout),'stderr_sha256':sha(q.stderr),'outcome':'restored-positive-pass'if kind=='restored'else'original-regression-detected'});print(kind,q.returncode,flush=True)
finally:
 for p,data in zip(files,original):p.write_bytes(data)
assert all(p.read_bytes()==data for p,data in zip(files,original))
(O/'report.json').write_text(json.dumps({'schema_version':1,'selected':4,'script_sha256':sha(Path(__file__).read_bytes()),'restored_source_sha256':{p.name:sha(p.read_bytes())for p in files},'family_certified':False,'rows':rows},indent=2)+'\n')
