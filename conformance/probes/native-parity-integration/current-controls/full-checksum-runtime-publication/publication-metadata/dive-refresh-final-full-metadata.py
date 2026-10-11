from pathlib import Path
import hashlib, json, subprocess, sys

scope = 'Published bounded Full/body-only editing with nested checksum expressions and CASE grouping repair; pure runtime-name checkpoint recovery; scoped final temporal checking. Original failed and successful campaigns remain immutable; no full-family certification.'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
for name in ['tlc-final-check', 'tlc-phase-execution', 'tlc-state-if', 'tlc-temporal-if', 'tlc-temporal-if-final', 'tlc-final-scopes', 'temporal-forwarding']:
    path = Path('conformance/probes') / name / 'source-matrix.json'
    data = json.loads(path.read_text())
    data['source_sha256'] = {n: sha(Path(n)) for n in data['source_sha256']}
    data['current_integration_scope'] = scope
    path.write_text(json.dumps(data, indent=2) + '\n')
subprocess.run([sys.executable, '-I', '/tmp/dive-refresh-native-integration.py', scope], check=True)
