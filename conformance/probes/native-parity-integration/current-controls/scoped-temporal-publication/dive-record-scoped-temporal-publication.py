from pathlib import Path
import hashlib,json,gzip,subprocess,re,shutil
root=Path('/Users/jeff/code/dive-temporal-if-publication-work');source='70afaa8997ecca6145fb38df2077d3688a010c91'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain','-uno'],cwd=root,text=True).strip()
out=root/'conformance/probes/native-parity-integration/current-controls/scoped-temporal-publication';out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();label='scoped-temporal-publication'
driver=Path('/tmp/dive-'+label+'-gate-driver.log');assert len(re.findall(r'^RESULT 0 ',driver.read_text(),re.M))==8
summary=Path('/tmp/dive-'+label+'-gate-2.log').read_text();assert re.search(r'957 tests run: 957 passed, 0 skipped',summary)
for i in range(8):shutil.copy2('/tmp/dive-'+label+'-gate-'+str(i)+'.log',out/('gate-'+str(i)+'.log'))
shutil.copy2(driver,out/'gate-driver.log')
for stem in ['build','inventory','controls','conformance','validation-ci']:
 shutil.copy2('/tmp/dive-'+label+'-'+stem+'.log',out/(stem+'.log'))
shutil.copy2('/tmp/dive-scoped-temporal-publication-combined-preflight.log',out/'source-preflight.log')
for name in ['dive-scoped-temporal-publication-controls.py','dive-publication-conformance.py','dive-publication-validation-ci.py','dive-record-scoped-temporal-publication.py']:shutil.copy2('/tmp/'+name,out/name)
controls=Path('/tmp/dive-scoped-temporal-publication-controls');c=json.loads((controls/'verification.json').read_text());assert c['source_commit']==source and [s['rows']for s in c['studies']]==[120,72] and len(c['cargo_owned_exact_guides'])==2
shutil.copytree(controls,out/'controls',ignore=shutil.ignore_patterns('conformance'))
conf=Path('/tmp/dive-scoped-temporal-publication-conformance');d=json.loads((conf/'verification.json').read_text());assert d['source_commit']==source and [r['selected_models']for r in d['controls']]==[21,16] and all(r['every_model_and_width_agrees']for r in d['controls']);shutil.copytree(conf,out/'conformance')
ci=Path('/tmp/dive-scoped-temporal-publication-validation-ci');d=json.loads((ci/'verification.json').read_text());assert d['source_commit']==source and d['controls'][0]['selected_models']==16 and d['controls'][0]['every_model_and_width_agrees'];shutil.copytree(ci,out/'validation-ci-bound')
inventory=json.loads(Path('/tmp/dive-'+label+'-inventory.log').read_text().splitlines()[-1]);assert inventory['selected_tests']==957 and inventory['ignored_tests']==0 and inventory['disjoint_and_complete']
inputs=json.loads((root/'conformance/probes/native-parity-integration/current-source.json').read_text())['source_sha256']
inputs.update(c['source_sha256'])
for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','.config/nextest.toml','clippy.toml','deny.toml']:inputs[name]=sha(root/name)
for name,digest in inputs.items():
 assert sha(root/name)==digest,name
 p=out/'producer-source'/(name+'.gz');p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(gzip.compress((root/name).read_bytes(),mtime=0))
receipt={'schema_version':1,'source_commit':source,'source_tree':subprocess.check_output(['git','rev-parse',source+'^{tree}'],cwd=root,text=True).strip(),'scope':'Current scoped LET/INSTANCE attribution, pure state/temporal IF planning, direct finite ~ENABLED safety routing and published spill/fairness/CI composition. Original92/f27 campaigns and failedc0 guard bindings remain immutable history.','source_sha256':inputs,'rust_tests':957,'rust_tests_skipped':0,'local_gate_stages_passed':8,'actual_test_partition_inventory':inventory,'cli_outcomes':192,'cli_workers':[1,3,4,8],'cargo_owned_exact_guides':2,'cm1_models':21,'validation_smoke_models':16,'gate_workers':[1,4,8],'validation_ci_timeout_seconds':30,'binary_sha256':json.loads((conf/'verification.json').read_text())['binary_sha256'],'family_certified':False,'ordinary_periodic_schedule_supported':False,'whole_corpus_rerun':False,'qualification':'Supported selected final-only fields and independently validated witnesses only. Historical full140 result remains6agree/134unsupported, not current-source corpus certification. Local contended timings are not performance or Linux CI claims.'}
receipt['evidence_sha256']={str(p.relative_to(out)):sha(p)for p in out.rglob('*')if p.is_file()};(out/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Recorded scoped temporal publication:',len(inputs),'source archives;',len(receipt['evidence_sha256']),'evidence files')
