#!/usr/bin/env python3
"""Bind process-owned source, complete selections, executables and passed gates."""
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
        else:assert b'683 tests run' in data and b'683 passed' in data and b'licenses ok' in data
        (gates/(label+'.log.gz')).write_bytes(gzip.compress(data,mtime=0))
    sources=[ROOT/p for p in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','CHANGELOG.md','conformance/parity-features.json','crates/dive-api/src/translate.rs','crates/dive-pluscal/src/lib.rs','crates/dive-pluscal/src/translate/procedures.rs','crates/dive-pluscal/src/translate/process.rs','crates/dive-pluscal/src/translate/emit.rs','crates/dive-pluscal/src/translate/lower.rs','crates/dive-pluscal/tests/process_procedures.rs','docs/decisions/0044-pluscal-process-procedure-ownership.md','docs/pluscal-process-procedures.md','docs/pluscal-recursive-procedures.md','docs/provenance.md','docs/divergences.md','docs/tlc-differences.md','docs/tlc-parity.md']]
    sources += [p for p in BASE.rglob('*') if p.suffix in ('.py','.java','.rs','.md') and '__pycache__' not in p.parts]
    files=[p for p in BASE.rglob('*') if p.is_file() and p.name!='delivery.json' and '__pycache__' not in p.parts]
    reports=[p for p in files if p.suffix=='.json']
    receipt={'schema_version':1,'parent_commit':'7102c566a8bc4d682da5a5ad6e88b8f982087992','native_sha256':sha(args.native),'example_sha256':sha(args.example),'harness_sha256':sha(args.harness),'native_profile':'debug; no performance claims','gate_status':'passed','cm1_ancestry_models':20,'parent_current_cm1_models':21,'family_certified':False,'tlc_violation_certified':False,'model_counts':{'selected_source_rows':130,'sany_cst_artifacts':80,'strict_complete_models':66,'strict_invariant_pending':62,'strict_deadlock_pending':2,'width_store_controls':780,'qualified_default_valid_witnesses':64,'qualified_default_invalid_mutants':256,'primary_sany_invalid_uncredited':14,'public_cli_admission_controls':28,'direct_api_admission_controls':28,'primary_timeout_prefixes':2,'native_bounded_controls':32},'reports':mapping(reports),'checked_sources':mapping(sources),'gate_logs':mapping(gates.glob('*.gz')),'archived_evidence':mapping(files)}
    (BASE/'delivery.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Bound',len(reports),'reports,',len(sources),'sources and',len(files),'archived files')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for key in ['native','example','harness','workspace','cm1','validation-smoke']:p.add_argument('--'+key,type=Path,required=True)
    main(p.parse_args())
