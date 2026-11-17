#!/usr/bin/env python3
"""Capture clean pinned DFID mode evidence without modifying the oracle."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

PIN = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'


def redactor(fixture, temp, jar):
    """Replace host paths with the harness placeholders ($TMP, $DIVE_CACHE_DIR, $CWD)."""
    replacements = [(str(Path(temp).resolve()), '$TMP'), (str(temp), '$TMP')]
    if jar.parent.parent.name == 'sha256':
        replacements.append((str(jar.parents[2]), '$DIVE_CACHE_DIR'))
    else:
        replacements.append((str(jar.parent), '$JAR_DIR'))
    replacements.append((str(fixture), '$CWD'))

    def redact(text):
        for path, placeholder in replacements:
            text = text.replace(path, placeholder)
        return text
    return redact


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--java', required=True, type=Path)
    parser.add_argument('--jar', required=True, type=Path)
    parser.add_argument('--reason', required=True)
    args = parser.parse_args()
    args.jar = args.jar.resolve()
    if hashlib.sha256(args.jar.read_bytes()).hexdigest() != PIN:
        raise SystemExit('pinned oracle SHA mismatch')
    fixture = Path(__file__).resolve().parent
    raw = fixture / 'raw'
    raw.mkdir(exist_ok=True)
    observations = []
    selection = []
    for model in sorted(fixture.glob('Dfid*.tla')):
        for depth in (0, 1, 2, 3, 4, 6):
            selection.append((model.stem, depth, 1, 7, 3, 0))
    selection += [('DfidCycle', 6, 0, 7, 3, 0), ('DfidCycle', 6, 2, 7, 3, 0), ('DfidLive', 6, 2, 7, 3, 0)]
    selection += [('DfidBranch', 6, 1, 7, 3, repeat) for repeat in range(1, 13)]
    selection += [('DfidRoots', 6, 1, 7, 3, repeat) for repeat in range(1, 7)]
    selection += [('DfidRootsMixed', 6, 1, 7, 3, repeat) for repeat in range(1, 13)]
    with tempfile.TemporaryDirectory(prefix='dive-dfid-primary-') as directory:
        redact = redactor(fixture, directory, args.jar)
        for model, depth, workers, seed, aril, repeat in selection:
            metadata = Path(directory) / f'{model}-{depth}-{workers}-{repeat}'
            metadata.mkdir()
            flags = ['-tool', '-workers', str(workers), '-fp', '0', '-seed', str(seed),
                     '-aril', str(aril), '-dfid', str(depth), '-metadir', str(metadata)]
            command = [str(args.java), f'-Djava.io.tmpdir={Path(directory).resolve()}',
                       '-cp', str(args.jar), 'tlc2.TLC', *flags,
                       '-config', f'{model}.cfg', f'{model}.tla']
            completed = subprocess.run(command, cwd=fixture, text=True, capture_output=True, timeout=30)
            completed.stdout, completed.stderr = redact(completed.stdout), redact(completed.stderr)
            control = f'{model}.depth{depth}.workers{workers}.seed{seed}.aril{aril}.repeat{repeat}'
            (raw / f'{control}.stdout').write_text(completed.stdout)
            (raw / f'{control}.stderr').write_text(completed.stderr)
            stats = re.findall(r'(\d+) states generated, (\d+) distinct states found\.', completed.stdout)
            observations.append(dict(control=control, model=model, depth=depth, workers=workers,
                                     seed=seed, aril=aril, repeat=repeat,
                                     flags=flags[:-1] + ['<temporary-metadata>'],
                                     exit=completed.returncode,
                                     codes=[int(code) for code in re.findall(r'STARTMSG (\d+):1', completed.stdout)],
                                     completion_message='STARTMSG 2193:' in completed.stdout,
                                     generated=int(stats[-1][0]) if stats else None,
                                     distinct=int(stats[-1][1]) if stats else None,
                                     trace_x=[int(value) for value in re.findall(r'(?:^|\n)(?:/\\ )?x = (-?\d+)', completed.stdout)]))
    files = [p for p in raw.rglob('*') if p.is_file()] + list(fixture.glob('Dfid*.tla')) + list(fixture.glob('Dfid*.cfg'))
    provenance = dict(schema_version=1, oracle='tlc-2026.10.06.014338', jar_sha256=PIN,
                      source_revision='94d0c5070852b37b801bb8998c436383ab7ee37b',
                      reason=args.reason, captured_at=datetime.now(timezone.utc).isoformat(),
                      java_executable_sha256=hashlib.sha256(args.java.read_bytes()).hexdigest(),
                      java_version=subprocess.run([str(args.java), '-version'], text=True, capture_output=True, check=True).stderr.strip(),
                      selected=len(selection), primary='clean unmodified TLC; no trace-dump flag or RNG injection',
                      observations=observations,
                      raw_files={p.relative_to(fixture).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)})
    (fixture / 'oracle-controls.json').write_text(json.dumps(provenance, indent=2) + '\n')
    print(f'captured {len(observations)} clean DFID controls')
    for name in ('DfidBranch', 'DfidRoots', 'DfidRootsMixed'):
        traces = {tuple(row['trace_x']) for row in observations if row['model'] == name and row['depth'] == 6 and row['workers'] == 1}
        print(name, 'fixed seed/aril trace variants:', sorted(traces))


if __name__ == '__main__':
    main()
