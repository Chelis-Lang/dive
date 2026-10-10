#!/usr/bin/env python3
"""Account every clean DFID control; unstable worker references remain ambiguous."""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess

def metadata_mismatches(native, process_exit):
    """Every disposition must retain honest proof and process metadata."""
    mismatch = []
    proved = native.get('result') == 'complete'
    if native.get('model_proved') is not proved or native.get('exhaustive') is not proved:
        mismatch.append('proof-scope')
    if native.get('exit_code') != process_exit:
        mismatch.append('process-exit')
    if native.get('result') == 'violation' and native.get('violation_validated') is not True:
        mismatch.append('independent-validation')
    if native.get('result') != 'violation' and native.get('violation_validated') is not False:
        mismatch.append('validation-scope')
    if native.get('result') == 'internal-error':
        mismatch.append('internal-error')
    return mismatch


def stable_mismatches(row, native, process_exit, ambiguous):
    """Worker choice never excuses false proof or uncertified violations."""
    mismatch = []
    expected_code = next((c for c in reversed(row['codes']) if c != 2121), 0)
    code = native.get('observed_tlc_code') or 0
    if code != expected_code:
        mismatch.append('deciding-code')
    if process_exit != row['exit']:
        mismatch.append('exit')
    if native.get('tlc_completion_message', False) != row['completion_message']:
        mismatch.append('completion-message')
    if native.get('result') == 'violation' and not native.get('violation_validated'):
        mismatch.append('independent-validation')
    proved = native.get('result') == 'complete'
    if native.get('model_proved') is not proved or native.get('exhaustive') is not proved:
        mismatch.append('proof-scope')
    expected_result = (
        'violation' if expected_code in (2107, 2108, 2110, 2112, 2114) else
        'parameter-error' if expected_code == 1102 else
        'evaluation-error' if expected_code else None
    )
    if expected_result and native.get('result') != expected_result:
        mismatch.append('stable-outcome')
    if not expected_code and native.get('result') not in ('incomplete', 'complete'):
        mismatch.append('stable-outcome')
    # This captured spec is an unbounded counter. No finite depth can prove it.
    if row['model'] == 'DfidBound' and native.get('result') != 'incomplete':
        mismatch.append('unbounded-model-proof')
    if ambiguous:
        # This ambiguous family is the curated Branch/Roots/RootsMixed group:
        # all references violate except Branch at depth 2, which reaches its
        # bound before any violation. Counts and witness choice may vary;
        # the deciding class and absence of a safety proof do not.
        expected_result = 'violation' if expected_code == 2110 else 'incomplete'
        if native.get('result') != expected_result:
            mismatch.append('stable-outcome')
        if native.get('model_proved') is not False or native.get('exhaustive') is not False:
            mismatch.append('proof-scope')
    if not ambiguous:
        for field in ('generated', 'distinct'):
            if row[field] is not None and native['statistics'][field] != row[field]:
                mismatch.append(field)
    return mismatch


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dive', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    a = parser.parse_args()
    base = Path(__file__).resolve().parent
    repository = base.parents[4]
    relative_fixture = base.relative_to(repository)
    reference = json.loads((base / 'oracle-controls.json').read_text())
    for path, digest in reference['raw_files'].items():
        if hashlib.sha256((base / path).read_bytes()).hexdigest() != digest:
            raise SystemExit(f'oracle fixture mismatch: {path}')
    rows = []
    for row in reference['observations']:
        run = subprocess.run([str(a.dive), 'dfid', str(relative_fixture / (row['model'] + '.tla')),
                              '--max-level', str(row['depth']), '--workers', str(row['workers']),
                              '--compat=tlc', '--seed', str(row['seed']), '--output', 'json'],
                             cwd=repository, text=True, capture_output=True, timeout=30)
        native = json.loads(run.stdout)
        mismatch = metadata_mismatches(native, run.returncode)
        if mismatch:
            outcome = 'disagree'
        elif native['result'] == 'unsupported':
            outcome = 'unsupported'
        elif native['result'] in ('rejected', 'input-error'):
            outcome = 'input-rejection-unverified-class'
        else:
            ambiguous = row['model'] in ('DfidBranch', 'DfidRoots', 'DfidRootsMixed') and row['depth'] >= 2
            mismatch = stable_mismatches(row, native, run.returncode, ambiguous)
            outcome = 'disagree' if mismatch else ('reference-ambiguous' if ambiguous else 'agree-documented-fields')
        rows.append(dict(control=row['control'], outcome=outcome, mismatches=mismatch,
                         native=native, process_exit=run.returncode))
    counts = dict(Counter(r['outcome'] for r in rows))
    report = dict(schema_version=1, command='dfid-primary-controls',
                  created_at=datetime.now(timezone.utc).isoformat(),
                  reference='clean unmodified pinned TLC; worker-selection ambiguity kept explicit',
                  reference_manifest_sha256=hashlib.sha256((base / 'oracle-controls.json').read_bytes()).hexdigest(),
                  implementation_sha256=hashlib.sha256(a.dive.read_bytes()).hexdigest(),
                  selected=len(rows), summary=counts,
                  comparison_fields=['deciding message code', 'generated', 'distinct', 'exit', 'completion message'],
                  limitations=['pure safety vertical slice only', 'raw text/collision probability output not certified',
                               'TLC worker randomness is not seeded by CLI; corresponding controls are reference-ambiguous',
                               'Ambiguity excludes counts/trace choice only; stable outcome/code/exit/validation/proof scope still checked',
                               'D-019 native witness correction never certifies malformed TLC words',
                               'binding failure class mapping pending in this API'],
                  models=rows)
    a.report.parent.mkdir(parents=True, exist_ok=True)
    a.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(counts, sort_keys=True))
    raise SystemExit(1 if counts.get('disagree') else 0)


if __name__ == '__main__':
    main()
