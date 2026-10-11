from pathlib import Path
import shutil,subprocess,json,hashlib,sys
kind=sys.argv[1];assert kind in ['base','nesting']
root=Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work');source='b8fb13cdab443e7ad0206a467d9a392075f31397'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
out=Path('/tmp/dive-full-runtime-cst-final-'+kind);out.mkdir(exist_ok=False)
folders=['primary','token-controls','control-shapes','context-controls','expression-controls']if kind=='base'else['primary','c-fixed-controls','layout-controls','separator-controls','empty-else-controls','terminal-empty-controls']
study=root/'crates/dive-pluscal'/('ast-checksum-acceptance'if kind=='base'else'checksum-expression-acceptance')
for name in folders:shutil.copytree(study/name,out/name)
strong=root/'crates/dive-pluscal/checksum-expression-acceptance/full-controls.py';script=strong.read_text()
if kind=='base':
 script=script.replace("['primary','c-fixed-controls','layout-controls','separator-controls','empty-else-controls','terminal-empty-controls']",repr(folders))
 script=script.replace("['P_elsif-two','P_fair_elsif-two','C_fixed_with-two','P_with-nested']","['P_assign','P_fair_assign','C_token_spaced','P_shape_while']")
(out/'full-controls.py').write_text(script)
jar=Path('/Users/jeff/code/dive-shipping-cache/sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar');java=Path('/Users/jeff/code/dive-shipping-cache/jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java');javac=Path('/tmp/claude-501/-Users-jeff-code-dive/7ef721df-a0bf-48ac-94b6-f7c0c8e766e7/scratchpad/jdk/jdk-17.0.20.1+1/Contents/Home/bin/javac')
shutil.copy2(root/'crates/dive-pluscal/rewrite-acceptance/ChecksumProbe.java',out/'ChecksumProbe.java');adapter=out/'adapter';adapter.mkdir()
p=subprocess.run([str(javac),'-cp',str(jar),'-d',str(adapter),str(out/'ChecksumProbe.java')],capture_output=True);(out/'javac.stdout').write_bytes(p.stdout);(out/'javac.stderr').write_bytes(p.stderr);assert p.returncode==0,p.stderr
artifacts=Path('/Users/jeff/code/dive-full-runtime-cst-final-artifacts')
cmd=['python3','-I',str(out/'full-controls.py'),'--java',str(java),'--jar',str(jar),'--native',str(artifacts/'dive'),'--editor',str(artifacts/'editor'),'--cst',str(artifacts/'cst'),'--validator',str(adapter)]
p=subprocess.run(cmd,cwd=root);assert p.returncode==0
report=json.loads((out/'full-controls/report.json').read_text());assert report['selected']==(216 if kind=='base' else 320)and len(report['corruption_controls'])==12
assert len(report['rows'])==report['selected'];assert all(r[profile].get('operator_structure_equal')is True for r in report['rows']for profile in ['dive','tlc']if r[profile]['outcome']=='full-derivation-and-admission')
if kind=='nesting':
 assert report['native_sany_and_validator_checks']==172
 for profile in ['dive','tlc']:assert report['summary'][profile]=={'full-derivation-and-admission':86,'named-refusal':140,'rejected':82,'primary-invalid-rejected':12},report['summary']
else:
 old=json.loads(Path('/tmp/dive-full-crc-publication-controls/full-controls/report.json').read_text());before={r['id']:r for r in old['rows']}
 for r in report['rows']:
  for profile in ['dive','tlc']:
   if before[r['id']][profile]['outcome']=='full-derivation-and-admission':assert r[profile]['outcome']=='full-derivation-and-admission',(r['id'],profile,'lost prior admission')
 assert report['native_sany_and_validator_checks']>=304
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
receipt={'source_commit':source,'kind':kind,'source_and_binary_producer':json.loads((artifacts/'producer.json').read_text()),'full_inputs':report['selected'],'sany_validator_complete_cst_pairs':report['native_sany_and_validator_checks'],'validator_corruptions':12,'strong_comparator_authority_sha256':sha(strong),'scoped_selector_changes':kind=='base','qualification':'Original full216 failing CST producer remains separate. No admission credit without exact whole-module node/token CST, native SANY and pinned Validator; complete selected refusals/rejections retained. No full-family certification.'}
receipt['evidence_sha256']={str(p.relative_to(out)):sha(p)for p in out.rglob('*')if p.is_file()};(out/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n');print('PASS final'+kind,report['summary'])
