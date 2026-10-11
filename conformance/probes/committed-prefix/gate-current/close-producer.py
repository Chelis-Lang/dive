from pathlib import Path
import json,hashlib,gzip,subprocess,shutil
root=Path.cwd();base=root/'conformance/probes/committed-prefix';gate=base/'gate-current'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();load=lambda p:json.loads(p.read_text())
before=load(gate/'before.json');after=load(gate/'after.json');runtime=load(gate/'runtime-closed.json');gate_producer=before['producer'];current=load(gate/'runtime-before.json');producer=current['producer'];qualification=load(gate/'final-documentation-closed.json')
assert gate_producer=='ed31d97378d43744c89a2bac2a48307ca10251ac'
assert qualification['only_doc_lines_changed'] and qualification['build_inputs_unchanged'] and qualification['production_commit']==producer
assert all(step['exit']==0 for step in qualification['steps'])
assert after['all_steps_passed']and after['build_inputs_unchanged']and after['expected_steps']==43
assert runtime['source_and_binaries_unchanged']and runtime['production_commit']==producer
assert all(r['exit']==0 for r in load(gate/'steps.json'))and all(r['exit']==0 for r in runtime['steps'])
assert sha(gate/'workspace.log')==after['log_sha256']
assert '1032 tests run: 1032 passed, 0 skipped'in(gate/'workspace.log').read_text()
inputs=before['source_sha256'];blob=subprocess.check_output(['git','cat-file','--batch'],input=''.join(gate_producer+':'+p+'\n'for p in inputs).encode());offset=0
for path,digest in inputs.items():
 end=blob.index(b'\n',offset);header=blob[offset:end].split();assert header[1]==b'blob';size=int(header[2]);data=blob[end+1:end+1+size];offset=end+size+2
 assert hashlib.sha256(data).hexdigest()==digest,path
 assert gzip.decompress((gate/'source'/(path+'.gz')).read_bytes())==data,path
assert offset==len(blob)
current_inputs=current['source_sha256'];assert set(current_inputs)==set(inputs)
changed=[p for p in inputs if inputs[p]!=current_inputs[p]];assert changed==['crates/dive-check/src/capture.rs']
strip=lambda x:b'\n'.join(l for l in x.splitlines()if not l.lstrip().startswith(b'///'))
assert strip(gzip.decompress((gate/'source'/(changed[0]+'.gz')).read_bytes()))==strip((root/changed[0]).read_bytes())
for path,digest in current_inputs.items():
 data=subprocess.check_output(['git','show',producer+':'+path]);assert hashlib.sha256(data).hexdigest()==sha(root/path)==digest,path
 assert gzip.decompress((gate/'runtime-source'/(path+'.gz')).read_bytes())==data,path
frozen=Path('/tmp/dive-committed-prefix-frozen-native')
assert all(sha(frozen/n)==h for n,h in runtime['binary_sha256'].items())
expected={f'candidate-w{w}.json'for w in [1,3,4,8]}|{f'complete-w{w}.json'for w in [1,4,8]}|{f'cut-w{w}-chaos{c}.json'for w in [1,3,4,8]for c in [0,3,7,19]}|{f'eof-{profile}-w{w}.json'for profile in ['Tlc','Dive']for w in [1,3,4,8]}|{f'eof-replay-{profile}-w{w}.json'for profile in ['Tlc','Dive']for w in [1,3,4,8]}|{f'published-{store}-w{w}.json'for store in ['Exact','Fingerprint']for w in [1,3,4,8]}|{f'safety-{label}-w{w}.json'for label in ['constraint','action-constraint','invariant','action-property','deadlock','predicate-error']for w in [1,3,4,8]}|{f'prefix-candidate-{n}-guarded{g}-w{w}.json'for n,g in [(1024,'false'),(8192,'false'),(1024,'true')]for w in [1,3,4,8]}
rows=[];valid=[];refused=[];published=0
for epoch in ['native-current','native-runtime']:
 paths=[p for p in (base/epoch).glob('*.json') if p.name!='manifest.json'];names={p.name for p in paths};quota={n for n in names if n.startswith('quota-')};assert len(quota)==1 and names==expected|quota,(epoch,names-expected,expected-names)
 assert len(paths)==84
 for p in paths:
  row=load(p);assert not row['model_proof']and not row['ordinary_schedule_certified']
  if p.name.startswith('eof-replay'):
   assert row['memory']==row['json']=='Valid'
   assert all(row[k].startswith('Invalid(')for k in ['wrong_init','wrong_property','wrong_closing_next','malformed_ending'])
   assert 'Next'in row['wrong_closing_next'];valid.append(str(p.relative_to(base)))
  else:
   rows.append(str(p.relative_to(base)))
   for pause in row['pauses']:
    assert pause['frontier_appended_start']==pause['committed_start']and pause['frontier_appended_end']==pause['committed_end']
    assert pause['predicate_workers']<=1
    committed=set(range(pause['committed_start'],pause['committed_end']))
    assert committed<=set(pause['completed'])<=set(pause['claimed'])
    assert pause['phase_l_written']==bool(committed)and not pause['model_proof']
    if p.name.startswith('published-'):published+=bool(committed)
   if p.name.startswith('cut-'):assert row['requests_cancelled']and row['eof']is None and not row['complete']
   if p.name.startswith('complete-')or p.name.startswith('published-'):assert row['complete']and row['eof']is not None
   if p.name.startswith('quota-'):assert not row['complete']and row['stop']is not None
assert published>0
for epoch in ['native-snapshot-current','native-runtime-snapshot']:
 for label in ['prefix-guarded','prefix-within-budget']:
  for w in [1,3,4,8]:
   p=base/epoch/f'{label}-w{w}.json';row=load(p);assert row['memory']==row['json']=='Valid'
   assert all(row[k].startswith('Invalid(')for k in ['wrong_init','wrong_property','wrong_closing_next','malformed_ending'])
   valid.append(str(p.relative_to(base)))
 for w in [1,3,4,8]:
  p=base/epoch/f'prefix-validator-refused-w{w}.json';row=load(p)
  assert all(row[k].startswith('Unsupported(')and'quantifier domain materialization work'in row[k]for k in ['memory','json'])
  refused.append(str(p.relative_to(base)))
manifest=dict(production_commit=producer,epoch_producers={'native-current':gate_producer,'native-snapshot-current':gate_producer,'native-runtime':producer,'native-runtime-snapshot':producer},rows=rows,valid_replays=valid,refused_replays=refused,ordinary_schedule_certified=False,model_proof=False,artifact_sha256={str(p.relative_to(base)):sha(p)for e in ['native-current','native-runtime','native-snapshot-current','native-runtime-snapshot']for p in sorted((base/e).glob('*.json')) if p.name!='manifest.json'})
(base/'native-current/manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
for epoch in ['replay-old-timer','replay-new-request']:
 paths=list((base/epoch).glob('*.json'));assert len(paths)==4
 for p in paths:
  row=load(p);assert row['memory']==row['json']=='Valid'
  assert all(row[k].startswith('Invalid(')for k in ['wrong_init','wrong_property','wrong_closing_next','malformed_ending'])
for n in ['gate','runtime']:
 shutil.copy2('/tmp/dive-committed-prefix-'+n+'.py',gate/(n+'-producer.py'))
 shutil.copy2('/tmp/dive-committed-prefix-'+n+'-launcher.log',gate/(n+'-launcher.log'))
shutil.copy2('/tmp/dive-committed-prefix-final-checks.py',gate/'final-documentation-producer.py')
shutil.copy2('/tmp/dive-committed-prefix-final-checks-launcher.log',gate/'final-documentation-launcher.log')
readback={'production_commit':producer,'full_gate_production_commit':gate_producer,'final_doc_only_qualification':qualification,'closed_consumers':True,'rust_cargo_git_live_gzip_pairs':len(inputs),'workspace_stages':43,'workspace_tests':1032,'zero_skip':True,'reference_method_rows':24,'reference_scope':'Real method/model controls with explicitly controlled injections; not ordinary native parity','native_request_rows_per_epoch':76,'isolated_timer_request_replays':8,'fresh_eof_replays_per_epoch':8,'fresh_prefix_replays_per_epoch':8,'refused_prefix_replays_per_epoch':4,'request_replay_scope':'new-request4+EOF8+bounded-prefix8 fresh Valid memory/JSON; original-prefix4 refuse unchanged independent-work bound; old-timer4 separate; repeated epochs are not summed as model coverage','required_models':{'examples':21,'validation-smoke':16,'workers':[1,4,8]},'immutable_binary_sha256':runtime['binary_sha256'],'ordinary_schedule_certified':False,'model_proof':False}
(gate/'closed-consumer-readback.json').write_text(json.dumps(readback,indent=2)+'\n')
(base/'DELIVERY.md').write_text('# Closed committed-prefix diagnostic\n\nFinal runtime producer is `'+producer+'`. The immutable full gate43 stages/1032 zero-skip tests belongs to earlier `'+gate_producer+'`; only capture.rs rustdoc captions changed before separately verified final-source documentation and runtime consumers, fresh Cargo guide,84 native rows per epoch (76 request facts plus8 independent EOF replays),8 valid bounded prefix replays and4 explicit replay-bound refusals per epoch. All37 required safety models agree at1/4/8. Primary24 actual controlled Worker/queue observations remain separate from native runtime. Neither repeated epochs nor graph/count agreement certifies ordinary scheduling or a model proof.\n\n[User contract](../../../docs/committed-periodic-prefix.md), [closed gate](gate-current/after.json), [runtime](gate-current/runtime-closed.json), [delivery index](delivery-index.json).\n')
receipts=[str(p.relative_to(root))for p in [gate/'before.json',gate/'after.json',gate/'steps.json',gate/'runtime-closed.json',gate/'runtime-steps.json',gate/'closed-consumer-readback.json',gate/'runtime-before.json',gate/'final-documentation-closed.json',base/'primary-index.json',base/'source-matrix.json']]
index={'schema_version':1,'production_commit':producer,'qualification':'Full gate and final runtime are distinct committed epochs; capture.rs changed only /// captions, with separate source/rustdoc/focused checks. Both producers unchanged during each closed consumer. Requests remain diagnostic; selected/committed/appended facts separate. Metadata bookkeeping is additive, no ordinary scheduling/verdict/family credit.','full_gate_production_commit':gate_producer,'rust_cargo_source_sha256':current_inputs,'full_gate_rust_cargo_source_sha256':inputs,'production_current_source_sha256':load(base/'source-matrix.json')['source_sha256'],'receipts':receipts,'artifact_sha256':{str(p.relative_to(root)):sha(p)for p in sorted(base.rglob('*'))if p.is_file()and p.name!='delivery-index.json'},'binary_sha256':runtime['binary_sha256'],'binary_names':list(runtime['binary_sha256']),'ordinary_schedule_certified':False,'model_proof':False}
(base/'delivery-index.json').write_text(json.dumps(index,indent=2)+'\n')
print('Closed producer readback:',len(inputs),'exact Git/live/gzip pairs',len(index['artifact_sha256']),'artifact hashes3bins; delivery index',sha(base/'delivery-index.json'))
