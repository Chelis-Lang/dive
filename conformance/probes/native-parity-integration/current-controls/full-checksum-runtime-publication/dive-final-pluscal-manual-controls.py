from pathlib import Path
import hashlib, json, os, subprocess

root = Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work')
source = 'b8fb13cdab443e7ad0206a467d9a392075f31397'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == source
binary = Path('/Users/jeff/code/dive-full-runtime-cst-final-artifacts/dive')
out = Path('/tmp/dive-full-runtime-cst-final-user-manual')
out.mkdir(exist_ok=False)
path = root / 'docs/user/examples/Algorithm.tla'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
before = sha(path)
env = os.environ.copy()
for name in ['DIVE_COMPAT', 'DIVE_CHAOS']:
    env.pop(name, None)
rows = []
for profile in ['dive', 'tlc']:
    for policy in ['full', 'tla']:
        args = ['translate', str(path), '--rewrite', '--checksums', policy, '--output', 'json']
        if profile == 'tlc':
            args.append('--compat=tlc')
        result = subprocess.run([str(binary), *args], cwd=root, env=env, capture_output=True, timeout=30)
        label = profile + '-' + policy
        (out / (label + '.stdout')).write_bytes(result.stdout)
        (out / (label + '.stderr')).write_bytes(result.stderr)
        data = json.loads(result.stdout)
        assert result.returncode == 0 and data['accepted']
        assert data['result'] == 'replacement' and data['model_proved'] is False and data['exhaustive'] is False
        assert data['algorithm_checksum_verified'] == (policy == 'full')
        assert data['body_checksum_verified'] is True
        assert data['profile'] == ('tlc+overrides' if profile == 'tlc' and policy == 'tla' else profile)
        assert sha(path) == before
        rows.append({'profile': profile, 'policy': policy, 'exit_code': result.returncode, 'command': args})
receipt = {'source_commit': source, 'source_sha256': before, 'binary_sha256': sha(binary), 'controls': rows, 'family_certified': False, 'filesystem_write': False}
receipt['evidence_sha256'] = {p.name: sha(p) for p in out.iterdir() if p.is_file()}
(out / 'verification.json').write_text(json.dumps(receipt, indent=2) + '\n')
print('4 exact current user-manual CLI/profile/policy controls passed; source unchanged')
