from pathlib import Path
import json, subprocess, hashlib

out = Path('/tmp/dive-terminal-classification-root-fresh')
out.mkdir(exist_ok=False)
bins = Path('/tmp/dive-terminal-classification-frozen-native')
producer = '3968821ee1dedf9a486896443a16f3af7e348570'
closed = json.loads(Path('/tmp/dive-terminal-classification-fullgate-retry1/runtime-current/runtime-closed.json').read_bytes())
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert closed['production_commit'] == producer
for n,h in closed['binary_sha256'].items(): assert sha(bins/n) == h
captures = []
def run(command, stem):
    result = subprocess.run(command, capture_output=True)
    (out/(stem+'.stdout')).write_bytes(result.stdout)
    (out/(stem+'.stderr')).write_bytes(result.stderr)
    captures.append({'command':command, 'exit':result.returncode, 'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
    return result
result = run([str(bins/'terminal-outcome-runtime'),str(out/'native')], 'native-campaign')
assert result.returncode == 0
rows = sorted((out/'native').glob('*.json')); assert len(rows) == 206
baseline = Path('/tmp/dive-terminal-classification-fullgate-retry1/runtime-current/native-current')
valid = invalid = 0
for file in rows:
    doc = json.loads(file.read_bytes())
    assert doc['model_proof'] is False and doc['ordinary_schedule_certified'] is False
    assert doc == json.loads((baseline/file.name).read_bytes()), file.name
    if 'document' not in doc: continue
    folder = out/file.stem; folder.mkdir()
    # Bind each word again through the independent public CLI, using its actual
    # module source and configuration. No backend Valid field grants admission.
    import re
    module = re.search(r'\bMODULE\s+([A-Za-z][A-Za-z0-9_]*)',doc['source'])[1]
    spec = folder/(module+'.tla'); spec.write_text(doc['source'])
    config = folder/(module+'.cfg'); config.write_text(doc['configuration'])
    words = [('original',doc['document'])]+[(m['kind'],m['document']) for m in doc['mutants']]
    for label,word in words:
        path = folder/(label+'.json'); path.write_text(json.dumps(word))
        command = [str(bins/'dive'),'validate-trace',str(spec),'--config',str(config),'--trace',str(path),'--compat=tlc','--seed','0','--fp','0','--output','json']
        r = run(command,file.stem+'-'+label)
        report = json.loads(r.stdout)
        if label == 'original':
            assert r.returncode == 0 and report['result'] == 'valid', (file.name,label,report)
            valid += 1
        else:
            assert r.returncode == 1 and report['result'] == 'invalid', (file.name,label,report)
            invalid += 1
assert valid == 28 and invalid == 112
guide = run([str(bins/'terminal-outcome-guide')],'guide')
assert guide.returncode == 0
for n,h in closed['binary_sha256'].items(): assert sha(bins/n) == h
receipt = {'compiled_producer':producer,'runtime_rows':206,'native_rows_identical_to_original':True,'fresh_public_cli_valid':valid,'fresh_public_cli_invalid':invalid,'commands':captures,'binary_sha256':closed['binary_sha256'],'family_certified':False,'model_proved':False,'ordinary_schedule_certified':False,'scope':'Fresh frozen native206 plus independent public CLI rebinding/replay of28 genuine and112 altered words. Default/controlled request contracts stay separate; no ordinary scheduler or Holds claim.'}
(out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({k:receipt[k]for k in ['runtime_rows','fresh_public_cli_valid','fresh_public_cli_invalid']}))
