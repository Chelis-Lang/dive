#!/usr/bin/env python3
"""Bind the final immutable selection, source, executables and gate evidence."""
import argparse,hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main(args):
    gate=BASE/'gates';gate.mkdir(exist_ok=True)
    for name in ['gate','final-runtime','final-controls','cm1','validation-smoke']:
        source=args.logs/('dive-pluscal-procedure-fairness-'+name+'.log')
        assert source.is_file(),name
        (gate/(name+'.log')).write_bytes(source.read_bytes())
    complete=(gate/'gate.log').read_text()
    assert '691 tests run: 691 passed' in complete
    assert 'CHECKS PASSED' in complete
    controls=(gate/'final-controls.log').read_text()
    assert all(message in controls for message in ['Rejected 39 full-document corruptions','Rejected all 12 public admission controls','Passed all 12 direct API invalid-artifact controls','Passed four guide CLI controls','Passed both executable guide API examples','Rejected all60 timeout-row full-document corruptions'])
    runtime=(gate/'final-runtime.log').read_text()
    assert runtime.count('QUALIFIED EXIT 0')==4 and runtime.count('EXIT 1')==4
    assert 'gate: 20 of 20 models' in (gate/'cm1.log').read_text()
    assert 'gate: 16 of 16 models' in (gate/'validation-smoke.log').read_text()
    source_paths=['crates/dive-pluscal/src/translate/fairness.rs','crates/dive-pluscal/src/translate/emit.rs','crates/dive-pluscal/src/translate/mod.rs','crates/dive-pluscal/src/translate/procedures.rs','crates/dive-pluscal/tests/procedure_fairness.rs','crates/dive-pluscal/tests/process_procedures.rs','docs/pluscal-procedure-fairness.md','CHANGELOG.md','conformance/parity-features.json','crates/dive-api/src/translate.rs','crates/dive-pluscal/src/lib.rs','crates/dive-pluscal/src/translate/process.rs','crates/dive-pluscal/examples/translate.rs','crates/dive-pluscal/tests/multiple_procedures.rs','docs/decisions/0046-process-procedure-fairness.md','docs/divergences.md','docs/pluscal-process-procedures.md','docs/pluscal-multiple-procedures.md','docs/provenance.md','docs/tlc-differences.md','docs/tlc-parity.md']
    sources={p:sha(ROOT/p) for p in source_paths}
    for file in BASE.rglob('*'):
        if file.is_file() and file.suffix in ['.py','.rs','.java','.md'] and '__pycache__' not in file.parts:
            sources[str(file.relative_to(ROOT))]=sha(file)
    evidence={str(p.relative_to(ROOT)):sha(p) for p in BASE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.name!='delivery.json'}
    reports={p:h for p,h in evidence.items() if p.endswith('.json')}
    logs={str(p.relative_to(ROOT)):sha(p) for p in gate.glob('*.log')}
    receipt={'schema_version':1,'scope':'Bounded process-owned procedure fairness and modifiers; no full PlusCal certification. Canonical witness replay is qualified, never strict TLC credit.','parent_commit':'bc73172fa833f28033bf509c1010efbcd4328ff8','native_sha256':sha(args.native),'example_sha256':sha(args.example),'harness_sha256':sha(args.harness),'family_certified':False,'tlc_violation_certified':False,'gate_status':'passed','capture_rows':480,'source_selection':474,'artifacts':302,'artifact_followup_repeats':6,'invalid_primary_artifacts':6,'model_rows':118,'worker_store_controls':708,'strict_complete':59,'strict_invariant_pending':59,'strict_deadlock_pending':0,'canonical_valid':59,'canonical_invalid_mutants':236,'rust_tests':691,'ancestry_cm1_models':20,'validation_smoke_models':16,'reports':reports,'checked_sources':sources,'gate_logs':logs,'archived_evidence':evidence}
    (BASE/'delivery.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print('Bound',len(reports),'reports,',len(sources),'sources and',len(evidence),'evidence files')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['native','example','harness']:p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--logs',type=Path,default=Path('/tmp'))
    main(p.parse_args())
