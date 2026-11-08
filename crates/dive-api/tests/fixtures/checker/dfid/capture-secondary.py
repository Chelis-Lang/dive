#!/usr/bin/env python3
"""Secondary injected-worker-seed reference; never a clean TLC golden."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

PIN = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--java', type=Path, required=True)
p.add_argument('--javac', type=Path, required=True)
p.add_argument('--jar', type=Path, required=True)
p.add_argument('--reason', required=True)
a = p.parse_args()
if hashlib.sha256(a.jar.read_bytes()).hexdigest() != PIN:
    raise SystemExit('pinned oracle SHA mismatch')
base = Path(__file__).resolve().parent
raw = base / 'secondary-raw'
raw.mkdir(exist_ok=True)
models = ['DfidBound', 'DfidCycle', 'DfidBranch', 'DfidRoots', 'DfidDuplicate', 'DfidNoStates',
          'DfidDeadlock', 'DfidConstrained', 'DfidInitialOnly', 'DfidIncomplete',
          'DfidRootsMixed', 'DfidInitialIncomplete', 'DfidInitialPrecedence', 'DfidSeedNames']
rows = []
with tempfile.TemporaryDirectory(prefix='dive-dfid-secondary-') as directory:
    classes = Path(directory) / 'classes'
    classes.mkdir()
    subprocess.run([str(a.javac), '-cp', str(a.jar), '-d', str(classes), str(base / 'SeededDfid.java')], check=True)
    for model in models:
        for depth in (2, 3, 6):
            for seed in (0, 1, 7, -1):
                control = f'{model}.depth{depth}.workerSeed{seed}'
                meta = Path(directory) / control
                meta.mkdir()
                run = subprocess.run([str(a.java), '-cp', f'{classes}:{a.jar}', 'SeededDfid',
                                      str(base / model), str(depth), str(seed), str(meta)],
                                     cwd=directory, text=True, capture_output=True, timeout=30)
                (raw / f'{control}.stdout').write_text(run.stdout)
                (raw / f'{control}.stderr').write_text(run.stderr)
                stats = re.findall(r'(\d+) states generated, (\d+) distinct states found\.', run.stdout)
                code = re.search(r'SECONDARY_DECIDING_CODE (\d+)', run.stdout)
                rows.append(dict(control=control, model=model, depth=depth, worker_seed=seed,
                                 seed_injection='all iteration workers reseeded after constructor, before start',
                                 process_exit=run.returncode, code=int(code[1]) if code else None,
                                 generated=int(stats[-1][0]) if stats else None,
                                 distinct=int(stats[-1][1]) if stats else None,
                                 completion_message='STARTMSG 2193:' in run.stdout,
                                 trace_x=[int(v) for v in re.findall(r'(?:^|\n)(?:/\\ )?x = (-?\d+)', run.stdout)]))
files = [p for p in raw.rglob('*') if p.is_file()] + [base / 'SeededDfid.java']
report = dict(schema_version=1, evidence='secondary instrumented reference; NOT clean primary oracle',
              jar_sha256=PIN, source_revision='94d0c5070852b37b801bb8998c436383ab7ee37b',
              java_sha256=hashlib.sha256(a.java.read_bytes()).hexdigest(),
              javac_sha256=hashlib.sha256(a.javac.read_bytes()).hexdigest(),
              compiler_version=subprocess.run([str(a.javac), '-version'], text=True, capture_output=True, check=True).stdout.strip(),
              reason=a.reason, captured_at=datetime.now(timezone.utc).isoformat(), selected=len(rows), observations=rows,
              raw_files={p.relative_to(base).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)})
(base / 'secondary-controls.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'captured {len(rows)} separately identified secondary controls')
