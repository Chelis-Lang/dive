#!/usr/bin/env python3
"""Bind the frozen native recursive source, artifacts, evidence and passed gates."""
import argparse,gzip,hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def mapping(paths):return {str(p.relative_to(ROOT)):sha(p) for p in sorted(paths)}
def main(args):
    gates=BASE/'gates';gates.mkdir(exist_ok=True)
    for label,source in [('workspace',args.workspace),('cm1',args.cm1),('validation-smoke',args.validation_smoke)]:
        data=source.read_bytes()
        if label!='workspace':assert data.rstrip().endswith(b'gate: passed')
        else:assert b'licenses ok' in data and b'failures' not in data.split(b'Summary')[-1]
        (gates/(label+'.log.gz')).write_bytes(gzip.compress(data,mtime=0))
    sources=[ROOT/p for p in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','CHANGELOG.md','conformance/parity-features.json','crates/dive-pluscal/src/lib.rs','crates/dive-pluscal/src/translate/procedures.rs','crates/dive-pluscal/src/translate/emit.rs','crates/dive-pluscal/src/translate/lower.rs','crates/dive-pluscal/tests/procedures.rs','crates/dive-pluscal/tests/procedure_graph.rs','crates/dive-pluscal/tests/recursive_procedures.rs','docs/decisions/0043-pluscal-recursive-procedure-calls.md','docs/pluscal-recursive-procedures.md','docs/pluscal-procedure-graphs.md','docs/pluscal-procedures.md','docs/provenance.md','docs/divergences.md','docs/tlc-differences.md','docs/tlc-parity.md']]
    sources += [p for p in BASE.glob('*') if p.suffix in ('.py','.java','.md')]
    sources += [BASE/'budgets/probe.rs']
    files=[p for p in BASE.rglob('*') if p.is_file() and p.name!='delivery.json' and '__pycache__' not in p.parts]
    reports=[p for p in files if p.suffix=='.json']
    receipt={'schema_version':1,'parent_commit':'cb4b51ff84487ab459bb3af00c99cd3eded5e0c1','native_sha256':sha(args.native),'example_sha256':sha(args.example),'native_profile':'debug; no performance claims','gate_status':'passed','family_certified':False,'tlc_violation_certified':False,'model_counts':{'selected_source_rows':116,'sany_cst_artifacts':86,'strict_complete_models':72,'strict_invariant_pending':72,'strict_deadlock_pending':2,'width_store_controls':876,'qualified_default_valid_witnesses':74,'qualified_default_invalid_mutants':296,'qualified_default_deadlock_valid':2,'primary_sany_invalid_uncredited':10,'primary_timeout_prefixes':12,'native_bounded_controls':192},'reports':mapping(reports),'checked_sources':mapping(sources),'gate_logs':mapping(gates.glob('*.gz')),'archived_evidence':mapping(files)}
    (BASE/'delivery.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Bound',len(reports),'reports,',len(sources),'sources and',len(files),'archived files')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for key in ['native','example','workspace','cm1','validation-smoke']:p.add_argument('--'+key,type=Path,required=True)
    main(p.parse_args())
