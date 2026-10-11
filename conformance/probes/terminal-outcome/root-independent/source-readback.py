from pathlib import Path
import hashlib, json, re, subprocess

root = Path('/Users/jeff/code/dive-periodic-snapshot-work')
gate = Path('/tmp/dive-terminal-classification-fullgate-retry1')
producer = '3968821ee1dedf9a486896443a16f3af7e348570'
sha = lambda data: hashlib.sha256(data).hexdigest()
before = json.loads((gate/'before.json').read_bytes())
after = json.loads((gate/'after.json').read_bytes())
assert before['producer'] == after['producer'] == producer
assert before['source_sha256'] == after['source_sha256']
assert after['build_inputs_unchanged'] and after['all_steps_passed']
assert before['unresolved_include_macros'] == []
tracked = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', producer], cwd=root, text=True).splitlines()
compiled = {n for n in tracked if n.endswith('.rs') and (re.match(r'^crates/[^/]+/(src|tests|examples|benches)/', n) or n.startswith('xtask/src/') or re.match(r'^crates/[^/]+/build\.rs$', n))}
assert compiled <= set(before['source_sha256'])
proc = subprocess.Popen(['git', 'cat-file', '--batch'], cwd=root, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
blobs = {}
for name, digest in before['source_sha256'].items():
    proc.stdin.write((producer+':'+name+'\n').encode()); proc.stdin.flush()
    header = proc.stdout.readline().split(); assert header[1] == b'blob'
    data = proc.stdout.read(int(header[2])); assert proc.stdout.read(1) == b'\n'
    assert sha(data) == digest, name
    blobs[name] = data
proc.stdin.close(); assert proc.wait() == 0
literal = {}
for name in sorted(compiled):
    text = blobs[name].decode()
    calls = list(re.finditer(r'\b(include_str|include_bytes|include)!\s*\(\s*"([^"\\]+)"\s*\)', text))
    assert len(calls) == len(list(re.finditer(r'\b(?:include_str|include_bytes|include)!\s*\(', text))), name
    for call in calls:
        target = (Path(name).parent/call[2]).as_posix()
        # Normalize lexical '..' without depending on current worktree content.
        import posixpath
        target = posixpath.normpath(target)
        assert target in blobs
        literal.setdefault(target, []).append(name)
assert {n:sorted(v) for n,v in literal.items()} == {n:sorted(v) for n,v in before['literal_include_sources'].items()}
steps = json.loads((gate/'steps.json').read_bytes())
assert len(steps) == after['expected_steps'] == 49 and all(s['exit'] == 0 for s in steps)
log = (gate/'workspace.log').read_bytes(); assert sha(log) == after['log_sha256']
assert re.search(rb'1100 tests run:\s*1100 passed(?: \(\d+ slow\))?, 0 skipped', log)
runtime = json.loads((gate/'runtime-current/runtime-closed.json').read_bytes())
assert runtime['production_commit'] == producer and runtime['source_sha256'] == before['source_sha256']
assert runtime['source_and_binaries_unchanged'] and len(runtime['steps']) == 21 and all(s['exit'] == 0 for s in runtime['steps'])
binary_root = Path('/tmp/dive-terminal-classification-frozen-native')
assert len(runtime['binary_sha256']) == 4
for name,digest in runtime['binary_sha256'].items(): assert sha((binary_root/name).read_bytes()) == digest
rows = list((gate/'runtime-current/native-current').glob('*.json'))
assert len(rows) == 206
for path in rows:
    d = json.loads(path.read_bytes())
    assert d['model_proof'] is False and d['ordinary_schedule_certified'] is False, path.name
receipt = {'compiled_producer':producer, 'git_selected_inputs_verified':len(blobs), 'compiled_rust_sources_independently_selected':len(compiled), 'literal_sites_independently_reconstructed':sum(map(len,literal.values())), 'literal_targets':len(literal), 'full_gate_commands':49, 'nextest_passed':1100, 'skipped':0, 'runtime_commands':21, 'native_rows':206, 'binary_sha256':runtime['binary_sha256'], 'family_certified':False, 'ordinary_schedule_certified':False, 'scope':'Independent immutable Git/source, literal inventory, closed gate and frozen binary readback. Fresh independent runtime replay and final portable packet review remain separate.'}
Path('/tmp/dive-terminal-classification-root-source-readback.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({k:receipt[k] for k in ['git_selected_inputs_verified','compiled_rust_sources_independently_selected','literal_sites_independently_reconstructed','native_rows','nextest_passed']}))
