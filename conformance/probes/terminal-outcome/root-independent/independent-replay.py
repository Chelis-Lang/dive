from pathlib import Path
import json, re, subprocess, hashlib

out = Path('/tmp/dive-terminal-classification-root-independent-replay')
out.mkdir(exist_ok=False)
fresh = Path('/tmp/dive-terminal-classification-root-fresh/native')
old = Path('/tmp/dive-terminal-classification-fullgate-retry1/runtime-current/native-current')
bins = Path('/tmp/dive-terminal-classification-frozen-native')
closed = json.loads(Path('/tmp/dive-terminal-classification-fullgate-retry1/runtime-current/runtime-closed.json').read_bytes())
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
for n,h in closed['binary_sha256'].items(): assert sha(bins/n) == h
captures = []; observations = []
def inspect(value):
    if isinstance(value,str):
        value = re.sub(r'\belapsed: [0-9]+(?:\.[0-9]+)?(?:ns|µs|ms|s)', 'elapsed: <measured-time>', value)
        value = re.sub(r'\bmillis: [0-9]+', 'millis: <measured-time>', value)
        value = re.sub(r'\bnow: [0-9]+', 'now: <measured-time>', value)
        # These exact raw diagnostic observations depend on milliseconds being
        # zero. Retain them separately; never claim their schedule equivalence.
        value = re.sub(r'\bratio: \((?:0\.0|NaN), (?:0\.0|NaN)\)', 'ratio: <retained-clock-observation>', value)
        return value
    if isinstance(value,dict): return {k:inspect(v) for k,v in value.items()}
    if isinstance(value,list): return [inspect(v)for v in value]
    return value
valid = invalid = 0
rows = sorted(fresh.glob('*.json')); assert len(rows) == 206
for file in rows:
    doc = json.loads(file.read_bytes()); prior = json.loads((old/file.name).read_bytes())
    assert doc['model_proof'] is False and doc['ordinary_schedule_certified'] is False
    assert inspect(doc) == inspect(prior), file.name
    changed = {k:{'original':prior[k],'fresh':doc[k]} for k in doc if doc[k] != prior[k]}
    if changed: observations.append({'case':file.name,'changed_raw_observations':changed})
    if 'document' not in doc: continue
    # Words, sources, configuration, terminal disposition and original counters
    # receive exact comparison without the measured diagnostic projection.
    assert doc['document'] == prior['document']
    folder = out/file.stem; folder.mkdir()
    module = re.search(r'\bMODULE\s+([A-Za-z][A-Za-z0-9_]*)',doc['source'])[1]
    spec = folder/(module+'.tla'); spec.write_text(doc['source'])
    config = folder/(module+'.cfg'); config.write_text(doc['configuration'])
    words = [('original',doc['document'])]+[(m['kind'],m['document'])for m in doc['mutants']]
    for label,word in words:
        path = folder/(label+'.json'); path.write_text(json.dumps(word))
        command = [str(bins/'dive'),'validate-trace',str(spec),'--config',str(config),'--trace',str(path),'--compat=tlc','--seed','0','--fp','0','--output','json']
        r = subprocess.run(command,capture_output=True)
        stem = file.stem+'-'+label
        (out/(stem+'.stdout')).write_bytes(r.stdout);(out/(stem+'.stderr')).write_bytes(r.stderr)
        captures.append({'command':command,'exit':r.returncode,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
        report = json.loads(r.stdout)
        if label == 'original':
            assert r.returncode == 0 and report['result'] == 'valid', (file.name,label,report)
            valid += 1
        else:
            assert r.returncode == 1 and report['result'] == 'invalid', (file.name,label,report)
            invalid += 1
assert valid == 28 and invalid == 112
for n,h in closed['binary_sha256'].items(): assert sha(bins/n) == h
(out/'raw-clock-observations.json').write_text(json.dumps(observations,indent=2)+'\n')
receipt = {'compiled_producer':closed['production_commit'],'fresh_native_rows':206,'native_campaign_exit':0,'original_failed_whole_json_comparison_retained':'/tmp/dive-terminal-classification-root-fresh.log','raw_clock_observations_retained':True,'clock_fields_excluded_from_replay_contract':['elapsed','millis','now','only captured zero/NaN ratios'],'complete_clock_schedule_equivalence':False,'fresh_public_cli_valid':valid,'fresh_public_cli_invalid':invalid,'commands':captures,'binary_sha256':closed['binary_sha256'],'family_certified':False,'model_proved':False,'ordinary_schedule_certified':False,'scope':'Fresh native206 actual classifications and source/count/word contracts; independently public CLI rebound28 genuine and112 mutant words. Raw measured clock/ratio differences retained, not declared deterministic schedule evidence.'}
(out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({k:receipt[k]for k in ['fresh_native_rows','fresh_public_cli_valid','fresh_public_cli_invalid']}))
