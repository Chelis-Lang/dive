from pathlib import Path
import json,hashlib,gzip,shutil,subprocess,re
root=Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work');source='d7c4f5911a1e68d21a10f67bb3bee0546d6448c6'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain','-uno'],cwd=root,text=True).strip()
out=root/'conformance/probes/native-parity-integration/current-controls/full-checksum-runtime-before-cst-repair';out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();label='full-checksum-runtime-publication'
assert len(re.findall(r'^RESULT 0 ',Path('/tmp/dive-'+label+'-gate-driver.log').read_text(),re.M))==8
assert '977 tests run: 977 passed' in Path('/tmp/dive-'+label+'-gate-2.log').read_text()
for p in Path('/tmp').glob('dive-'+label+'*.log'):shutil.copy2(p,out/p.name)
for name in ['dive-full-crc-publication-controls','dive-current-full-publication-cst','dive-current-full-publication-cst-complete','dive-full-checksum-runtime-publication-full-guide','dive-full-checksum-runtime-publication-runtime-names','dive-full-checksum-runtime-publication-conformance','dive-full-checksum-runtime-publication-validation-ci']:
 src=Path('/tmp')/name;dest=out/name
 for p in src.rglob('*'):
  if p.is_file() and not p.is_symlink() and 'conformance' not in p.relative_to(src).parts:
   d=dest/p.relative_to(src);d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,d)
for name in ['dive-current-full-publication-cst.py','dive-current-full-publication-cst-complete.py','dive-current-full-publication-cst.log','dive-current-full-publication-cst-complete.log','dive-run-full-crc-publication-controls.py','dive-pluscal-full-publication-guide.py','dive-runtime-name-publication-controls.py','dive-record-full-publication-failed-cst.py']:shutil.copy2('/tmp/'+name,out/name)
inputs=json.loads((root/'conformance/probes/native-parity-integration/current-source.json').read_text())['source_sha256']
for p in (root/'crates').rglob('*'):
 if p.is_file() and p.suffix in ['.rs','.toml']:inputs[str(p.relative_to(root))]=sha(p)
for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','.config/nextest.toml','clippy.toml','deny.toml']:inputs[name]=sha(root/name)
for name,digest in inputs.items():
 assert sha(root/name)==digest,name
 p=out/'producer-source'/(name+'.gz');p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(gzip.compress((root/name).read_bytes(),mtime=0))
c=json.loads(Path('/tmp/dive-current-full-publication-cst-complete/verification.json').read_text());assert c['complete_module_pairs']==304 and len(c['mismatches'])==12
r={'schema_version':1,'source_commit':source,'source_sha256':inputs,'publication_status':'blocked-by-failed-strict-CST-controls','rust_tests':977,'rust_tests_skipped':0,'local_gate_stages_passed':8,'complete_cst_pairs':304,'complete_cst_mismatches':c['mismatches'],'sany_validator_pairs':304,'full_crc_inputs':216,'runtime_name_cli_controls':195,'cm1_models':21,'validation_smoke_models':16,'qualification':'Passing workspace/SANY/Validator/runtime-name controls do not excuse12 failed syntax-tree comparisons. Original binary, source, raw controls and failed prefix retained. No current-generation or family certification.'}
r['evidence_sha256']={str(p.relative_to(out)):sha(p)for p in out.rglob('*')if p.is_file()};(out/'verification.json').write_text(json.dumps(r,indent=2)+'\n');print('Preserved failed304-CST campaign,977-test gate and',len(inputs),'source archives')
