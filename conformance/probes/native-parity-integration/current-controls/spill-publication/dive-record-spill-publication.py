from pathlib import Path
import hashlib, json, gzip, shutil, subprocess, re

root=Path('/Users/jeff/code/dive-checkpoint-spill-publication-work')
source='fbaa81a11fc4a0f7a7606f7a951fb2b91b84384f'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain','-uno'],cwd=root,text=True).strip()
out=root/'conformance/probes/native-parity-integration/current-controls/spill-publication'
out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
driver=Path('/tmp/dive-spill-publication-gate-driver.log').read_text()
assert len(re.findall(r'^RESULT 0 ',driver,re.M))==8,driver
summary=Path('/tmp/dive-spill-publication-gate-2.log').read_text()
assert re.search(r'928 tests run: 928 passed .*0 skipped',summary),summary[-1000:]
inventory=json.loads(Path('/tmp/dive-spill-publication-inventory.log').read_text().splitlines()[-1])
assert inventory['selected_tests']==928 and inventory['ignored_tests']==0 and inventory['disjoint_and_complete']
for name in ['gate-driver','build','source-checks','guide','conformance','inventory']:
    path=Path('/tmp/dive-spill-publication-'+name+'.log')
    shutil.copy2(path,out/(name+'.log'))
for number in range(8):shutil.copy2(Path('/tmp/dive-spill-publication-gate-'+str(number)+'.log'),out/('gate-'+str(number)+'.log'))
for helper in ['dive-spill-publication-guide.py','dive-publication-conformance.py','dive-publication-validation-ci.py','dive-record-spill-publication.py']:
    shutil.copy2(Path('/tmp')/helper,out/helper)
initial=Path('/tmp/dive-spill-publication-initial-guide')
initial_dest=out/'initial-guide-runner-failure';initial_dest.mkdir()
for path in initial.rglob('*'):
    if path.is_file() and not path.is_symlink() and 'conformance' not in path.relative_to(initial).parts:
        dest=initial_dest/path.relative_to(initial);dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,dest)
shutil.copy2('/tmp/dive-spill-publication-guide-initial.py',initial_dest/'runner.py')
(initial_dest/'qualification.json').write_text(json.dumps({'scope':'Runner failure, not an API conformance disagreement.', 'cause':'CLI controls and the Rust example initially shared cp-spill.cp. The documented create-new/no-overwrite policy refused the already existing archive; the paused status assertion then failed.', 'correction':'Run the unchanged documented Rust example in a fresh runtime directory with a read-only conformance symlink. Original sources, stdout/stderr and archive are retained.', 'family_certified':False},indent=2)+'\n')
guide=Path('/tmp/dive-spill-publication-executed-guide')
guide_receipt=json.loads((guide/'verification.json').read_text())
assert guide_receipt['source_commit']==source and guide_receipt['rust_example_executed']
guide_dest=out/'guide';guide_dest.mkdir()
for path in guide.rglob('*'):
    if path.is_file() and not path.is_symlink() and 'conformance' not in path.relative_to(guide).parts:
        dest=guide_dest/path.relative_to(guide);dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,dest)
conformance=Path('/tmp/dive-spill-publication-conformance')
conformance_receipt=json.loads((conformance/'verification.json').read_text())
assert conformance_receipt['source_commit']==source
assert [c['selected_models'] for c in conformance_receipt['controls']]==[21,16]
assert all(c['every_model_and_width_agrees'] for c in conformance_receipt['controls'])
shutil.copytree(conformance,out/'conformance')
ci_validation=Path('/tmp/dive-spill-publication-validation-ci')
ci_validation_receipt=json.loads((ci_validation/'verification.json').read_text())
assert ci_validation_receipt['source_commit']==source
assert ci_validation_receipt['controls'][0]['selected_models']==16
assert ci_validation_receipt['controls'][0]['every_model_and_width_agrees']
assert '--timeout' in ci_validation_receipt['controls'][0]['command']
shutil.copytree(ci_validation,out/'validation-ci-bound')
shutil.copy2('/tmp/dive-spill-publication-validation-ci.log',out/'validation-ci-bound.log')
inputs=json.loads((root/'conformance/probes/native-parity-integration/current-source.json').read_text())['source_sha256']
for name,digest in inputs.items():
    path=root/name;assert sha(path)==digest,name
    dest=out/'producer-source'/(name+'.gz');dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(gzip.compress(path.read_bytes(),mtime=0))
receipt={'schema_version':1,'source_commit':source,
         'source_tree':subprocess.check_output(['git','rev-parse',source+'^{tree}'],cwd=root,text=True).strip(),
         'scope':'Current checkpoint transient-storage integration with process-procedure fairness and partitioned CI. Fresh current-source controls; earlier 7872bf6 campaign remains separately frozen.',
         'family_certified':False,'independent_archive_certificate':False,'java_archive_interoperability':False,
         'source_sha256':inputs,'rust_tests':928,'rust_tests_skipped':0,'local_gate_stages_passed':8,
         'actual_test_partition_inventory':inventory,'cli_guide_controls':3,'cargo_rust_guide_executed':True,
         'cm1_models':21,'validation_smoke_models':16,'workers':[1,4,8],
         'validation_smoke_ci_timeout_seconds':30,
         'binary_sha256':conformance_receipt['binary_sha256'],
         'prior_campaign':'conformance/probes/native-checkpoint-spill/current-shipping/delivery.json',
         'qualification':'No reuse of the earlier 4032 CLI campaign as current-source execution. Local durations under contended macOS load are retained, not a performance or Linux CI claim.',
         'evidence_sha256':{str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file()}}
(out/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n')
print('Recorded fresh spill publication:',len(inputs),'source inputs;',len(receipt['evidence_sha256']),'evidence files')
