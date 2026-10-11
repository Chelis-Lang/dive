from pathlib import Path
import subprocess, json, hashlib, os, sys, time

root, binaries, out = map(lambda s: Path(s).resolve(), sys.argv[1:4])
source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
out.mkdir(parents=True, exist_ok=False)
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
env = os.environ.copy()
env['DIVE_CACHE_DIR'] = '/Users/jeff/code/dive-shipping-cache'
for key in ['DIVE_COMPAT', 'DIVE_CHAOS']:
    env.pop(key, None)
records = []
for corpus, selection in [('examples', 'cm1'), ('validation-smoke', 'validation-smoke')]:
    model_list = root / 'conformance/gates' / (selection + '.txt')
    models = [line.split('#', 1)[0].strip() for line in model_list.read_text().splitlines()]
    models = [m for m in models if m]
    reports = out / selection
    command = [str(binaries / 'dive-conformance'), 'run', '--impl', 'dive', '--impl-profile', 'tlc',
               '--corpus', corpus, '--tier', 'all', '--no-sany', '--no-summary',
               '--workers', '1,4,8', '--dive-bin', str(binaries / 'dive'), '--report-dir', str(reports)]
    for model in models:
        command += ['--filter', model]
    start = time.monotonic()
    with (out / (selection + '.log')).open('wb') as log:
        result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=2400)
    record = {'source_commit': source, 'command': command, 'exit_code': result.returncode,
              'elapsed_seconds': time.monotonic() - start, 'selected_models': len(models),
              'selection_sha256': sha(model_list), 'workers': [1, 4, 8]}
    (out / (selection + '.command.json')).write_text(json.dumps(record, indent=2) + '\n')
    assert result.returncode == 0, record
    report_paths = list(reports.glob('*.json'))
    assert len(report_paths) == 1, report_paths
    report = json.loads(report_paths[0].read_text())
    assert report['settings']['workers'] == [1, 4, 8], report['settings']
    assert len(report['models']) == len(models), report['summary']
    assert report['summary']['models_selected'] == len(models), report['summary']
    for model in report['models']:
        assert model['outcome'] == 'agree', model
        assert sorted(run['workers'] for run in model['runs']) == [1, 4, 8], model
        assert all(run['outcome'] == 'agree' and not run['diffs'] for run in model['runs']), model
    record['every_model_and_width_agrees'] = True
    records.append(record)
    print(selection, len(models), 'agree at 1/4/8 workers', flush=True)
receipt = {'schema_version': 1, 'source_commit': source, 'family_certified': False,
           'binary_sha256': {name: sha(binaries / name) for name in ['dive', 'dive-conformance']},
           'controls': records,
           'files_sha256': {str(p.relative_to(out)): sha(p) for p in out.rglob('*') if p.is_file()}}
(out / 'verification.json').write_text(json.dumps(receipt, indent=2) + '\n')
