import pathlib, json, importlib.util, hashlib, copy
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work')
p=root/'conformance/probes/fallible-invariant-replay/current'
spec=importlib.util.spec_from_file_location('verify',root/'scripts/check-fallible-invariant-replay.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
sha=lambda b:hashlib.sha256(b).hexdigest()
index_raw=(p/'delivery-index.json').read_bytes(); original=json.loads(index_raw); results=[]
def control(name,file,edit):
 target=p/file; raw=target.read_bytes(); value=json.loads(raw);edit(value);target.write_text(json.dumps(value,indent=2)+'\n');index=copy.deepcopy(original)
 if file!='delivery-index.json':index['payload_sha256'][file]=sha(target.read_bytes());(p/'delivery-index.json').write_text(json.dumps(index,indent=2)+'\n')
 rejected=False
 try:module.verify(p)
 except (AssertionError,KeyError,ValueError):rejected=True
 finally:target.write_bytes(raw);(p/'delivery-index.json').write_bytes(index_raw)
 assert rejected,name;results.append({'control':name,'coherent_payload_hash':file!='delivery-index.json','rejected':True});print(name,'REJECTED',flush=True)
control('wrong-runtime-producer','delivery-index.json',lambda d:d.update(runtime_commit='0'*40))
control('wrong-original-source-sha','source-index.json',lambda d:next(iter(d['inputs'].values())).update(sha256='0'*64))
control('invented-pure-selection-count','replay-runtime/receipt.json',lambda d:d.update(pure_disk_selected=431))
control('omitted-embedded-target-reference','embedded-input-supplement/index.json',lambda d:d['references'].pop())
control('renamed-original-semantic-mutant','api-controls/stdout.json',lambda d:d['rows'][0]['mutants'][0].update(name='invented-mutant'))
restored=module.verify(p,binary_root=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-replay-current/current-binaries'),source_root=root)
pathlib.Path('/tmp/dive-replay-final-mutations.json').write_text(json.dumps({'runtime_commit':module.PRODUCER,'controls':results,'restored':restored},indent=2)+'\n')
print('RESTORED PASS',flush=True)
