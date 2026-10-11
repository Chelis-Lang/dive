from pathlib import Path
import gzip, hashlib, json, re, shutil, subprocess

root = Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work')
source = 'b8fb13cdab443e7ad0206a467d9a392075f31397'
label = 'full-runtime-cst-final'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == source
assert not subprocess.check_output(['git', 'status', '--porcelain', '-uno'], cwd=root, text=True).strip()
out = root / 'conformance/probes/native-parity-integration/current-controls/full-checksum-runtime-publication'
assert not out.exists()
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
driver = Path('/tmp/dive-' + label + '-gate-driver.log')
assert len(re.findall(r'^RESULT 0 ', driver.read_text(), re.M)) == 8
assert re.search(r'982 tests run: 982 passed(?: \([^\n]*\))?, 0 skipped', Path('/tmp/dive-' + label + '-gate-2.log').read_text())
inventory = json.loads(Path('/tmp/dive-' + label + '-inventory.log').read_text().splitlines()[-1])
assert inventory['selected_tests'] == 982 and inventory['ignored_tests'] == 0 and inventory['disjoint_and_complete']
controls = {}
for kind in ['base', 'nesting', 'runtime-names', 'full-guide', 'user-manual', 'conformance', 'validation-ci']:
    directory = Path('/tmp/dive-' + label + '-' + kind)
    receipt = json.loads((directory / 'verification.json').read_text())
    assert receipt['source_commit'] == source, kind
    for key in ['evidence_sha256', 'files_sha256']:
        for name, digest in receipt.get(key, {}).items():
            assert sha(directory / name) == digest, (kind, name)
    controls[kind] = receipt
assert controls['base']['sany_validator_complete_cst_pairs'] >= 304
assert controls['base']['full_inputs'] == 216
assert controls['nesting']['sany_validator_complete_cst_pairs'] == 172
assert controls['nesting']['full_inputs'] == 320
assert len(controls['runtime-names']['controls']) == 195 and controls['runtime-names']['exact_cargo_owned_guide']
assert len(controls['full-guide']['controls']) == 12 and controls['full-guide']['exact_cargo_owned_rust_guide']
assert len(controls['user-manual']['controls']) == 4 and controls['user-manual']['filesystem_write'] is False
assert [c['selected_models'] for c in controls['conformance']['controls']] == [21, 16]
assert all(c['every_model_and_width_agrees'] for c in controls['conformance']['controls'])
assert controls['validation-ci']['controls'][0]['selected_models'] == 16
assert controls['validation-ci']['controls'][0]['every_model_and_width_agrees']
out.mkdir(parents=True)
for p in Path('/tmp').glob('dive-' + label + '*.log'):
    shutil.copy2(p, out / p.name)
for kind in controls:
    directory = Path('/tmp/dive-' + label + '-' + kind)
    for p in directory.rglob('*'):
        if p.is_file() and not p.is_symlink() and 'conformance' not in p.relative_to(directory).parts:
            destination = out / kind / p.relative_to(directory)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(p, destination)
for name in ['dive-run-ci-gate.py', 'dive-check-source-controls.py', 'dive-build-final-full-runtime-artifacts.py', 'dive-run-final-full-controls.py', 'dive-runtime-name-final-controls.py', 'dive-pluscal-full-final-guide.py', 'dive-final-pluscal-manual-controls.py', 'dive-publication-conformance.py', 'dive-publication-validation-ci.py', 'dive-record-final-full-runtime-publication.py']:
    shutil.copy2('/tmp/' + name, out / name)
inputs = json.loads((root / 'conformance/probes/native-parity-integration/current-source.json').read_text())['source_sha256']
for p in (root / 'crates').rglob('*'):
    if p.is_file() and p.suffix in ['.rs', '.toml']:
        inputs[str(p.relative_to(root))] = sha(p)
for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.config/nextest.toml', 'clippy.toml', 'deny.toml']:
    inputs[name] = sha(root / name)
for name, digest in inputs.items():
    assert sha(root / name) == digest, name
    destination = out / 'producer-source' / (name + '.gz')
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(gzip.compress((root / name).read_bytes(), mtime=0))
receipt = {
    'schema_version': 1,
    'source_commit': source,
    'source_tree': subprocess.check_output(['git', 'rev-parse', source + '^{tree}'], cwd=root, text=True).strip(),
    'source_sha256': inputs,
    'scope': 'Bounded Full/BodyOnly PlusCal editing, nested checksum expressions and corrected CASE grouping, composed with pure runtime-name checkpoint recovery and shipped scoped temporal checking.',
    'rust_tests': 982, 'rust_tests_skipped': 0, 'local_gate_stages_passed': 8,
    'actual_test_partition_inventory': inventory,
    'full_base_inputs': 216, 'full_nested_inputs': 320,
    'full_base_complete_cst_pairs': controls['base']['sany_validator_complete_cst_pairs'],
    'full_nested_complete_cst_pairs': 172, 'validator_corruptions': 24,
    'runtime_name_cli_controls': 195, 'full_checksum_cli_guide_controls': 12,
    'current_user_manual_cli_controls': 4,
    'cargo_owned_exact_guides': 2,
    'cm1_models': 21, 'validation_smoke_models': 16, 'gate_workers': [1, 4, 8],
    'validation_ci_timeout_seconds': 30,
    'artifact_producer': json.loads(Path('/Users/jeff/code/dive-full-runtime-cst-final-artifacts/producer.json').read_text()),
    'family_certified': False, 'whole_corpus_rerun': False,
    'independent_tlc_runtime_name_certificate': False,
    'qualification': 'The old d7 977-test/304-pair campaign remains blocked by 12 complete-CST mismatches and is preserved separately. These fresh controls prove only admitted bounded artifacts and recovery cases. No formatter, filesystem rewrite, mapping-format or full-family certification; contended local timings are not performance claims.'
}
receipt['evidence_sha256'] = {str(p.relative_to(out)): sha(p) for p in out.rglob('*') if p.is_file()}
(out / 'verification.json').write_text(json.dumps(receipt, indent=2) + '\n')
for name, digest in receipt['evidence_sha256'].items():
    assert sha(out / name) == digest, name
for name, digest in inputs.items():
    assert hashlib.sha256(gzip.decompress((out / 'producer-source' / (name + '.gz')).read_bytes())).hexdigest() == digest, name
print('Recorded and read back', len(inputs), 'producer archives and', len(receipt['evidence_sha256']), 'evidence hashes')
