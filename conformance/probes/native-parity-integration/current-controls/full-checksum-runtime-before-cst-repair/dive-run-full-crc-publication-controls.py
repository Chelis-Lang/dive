from pathlib import Path
import shutil,subprocess,json,hashlib,sys
root=Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work');out=Path('/tmp/dive-full-crc-publication-controls');out.mkdir(exist_ok=False)
study=root/'crates/dive-pluscal/ast-checksum-acceptance'
for name in ['primary','token-controls','control-shapes','context-controls','expression-controls']:shutil.copytree(study/name,out/name)
shutil.copy2(study/'full-controls.py',out/'full-controls.py')
jar=Path('/Users/jeff/code/dive-shipping-cache/sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar')
java=Path('/Users/jeff/code/dive-shipping-cache/jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java')
javac=Path('/tmp/claude-501/-Users-jeff-code-dive/7ef721df-a0bf-48ac-94b6-f7c0c8e766e7/scratchpad/jdk/jdk-17.0.20.1+1/Contents/Home/bin/javac')
assert javac.is_file()
source=root/'crates/dive-pluscal/rewrite-acceptance/ChecksumProbe.java';shutil.copy2(source,out/'ChecksumProbe.java');adapter=out/'adapter';adapter.mkdir()
command=[str(javac),'-cp',str(jar),'-d',str(adapter),str(out/'ChecksumProbe.java')]
r=subprocess.run(command,capture_output=True);(out/'javac.stdout').write_bytes(r.stdout);(out/'javac.stderr').write_bytes(r.stderr);assert r.returncode==0,r.stderr
artifacts=Path('/Users/jeff/code/dive-full-checksum-runtime-publication-artifacts')
cmd=['python3','-I',str(out/'full-controls.py'),'--java',str(java),'--jar',str(jar),'--native',str(artifacts/'dive'),'--editor',str(artifacts/'editor'),'--validator',str(adapter)]
r=subprocess.run(cmd,cwd=root);assert r.returncode==0
report=json.loads((out/'full-controls/report.json').read_text());assert report['selected']==216 and report['native_sany_and_validator_checks']==304 and len(report['corruption_controls'])==12
for profile in ['dive','tlc']:assert report['summary'][profile]=={'full-derivation-and-admission':152,'rejected':24,'named-refusal':36,'primary-invalid-rejected':4},report['summary']
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
receipt={'schema_version':1,'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'source_and_binary_producer':json.loads((artifacts/'producer.json').read_text()),'fresh_controls':216,'profile_controls':432,'sany_validator_pairs':304,'validator_corruptions':12,'family_certified':False,'primary_invalid_uncredited':True,'adapter_source_sha256':sha(source),'javac_sha256':sha(javac),'java_sha256':sha(java),'jar_sha256':sha(jar),'source_fixture_sha256':{str(p.relative_to(study)):sha(p)for name in ['primary','token-controls','control-shapes','context-controls','expression-controls']for p in (study/name).rglob('*')if p.is_file()}}
assert receipt['source_commit']=='d7c4f5911a1e68d21a10f67bb3bee0546d6448c6'
receipt['evidence_sha256']={str(p.relative_to(out)):sha(p)for p in out.rglob('*')if p.is_file()};(out/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Fresh complete216 Full controls,304 SANY/Validator pairs and12 corruptions passed')
