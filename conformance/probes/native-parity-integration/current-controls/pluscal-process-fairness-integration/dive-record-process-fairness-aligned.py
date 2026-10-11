#!/usr/bin/env python3
"""Bind actual combined-source controls and logs without promoting old proofs."""
import hashlib,importlib.util,json,re,subprocess
from pathlib import Path
spec=importlib.util.spec_from_file_location('fresh_controls','/tmp/dive-process-fairness-current-controls.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
GATE_SOURCE='f3a588ff057322d5b4f154dfabd2ad2ac2b888ac'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=m.ROOT,text=True).strip()==GATE_SOURCE
assert subprocess.run(['git','diff','--exit-code',GATE_SOURCE,'--','*.rs','Cargo.toml','Cargo.lock'],cwd=m.ROOT,stdout=subprocess.DEVNULL).returncode==0
driver=Path('/tmp/dive-pluscal-process-fairness-aligned-gate-driver.log').read_text()
assert driver.count('RESULT 0 ')==8 and 'RESULT 100 ' not in driver
a=m.OUT/'workspace-gate-driver.log';a.write_text(driver)
control=m.OUT/'source-checks-aligned.log';control.write_bytes(Path('/tmp/dive-pluscal-process-fairness-aligned-source-checks.log').read_bytes())
commands=[['cargo','fmt','--all','--check'],['cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings'],['cargo','nextest','run','--workspace','--locked'],['cargo','test','--workspace','--doc','--locked'],['cargo','doc','--workspace','--no-deps','--locked'],['cargo','xtask','check-layering'],['cargo','build','-p','dive-trace','--locked'],['cargo','deny','--locked','check']]
logs={}
for i,cmd in enumerate(commands):
    path=Path('/tmp/dive-pluscal-process-fairness-aligned-gate-'+str(i)+'.log');assert path.exists(),path
    destination=m.OUT/('workspace-gate-'+str(i)+'.log');destination.write_bytes(path.read_bytes());logs[str(i)]={'command':cmd,'log':destination.name,'sha256':m.sha(destination)}
nextest=(m.OUT/'workspace-gate-2.log').read_text();match=re.search(r'Summary.*?([0-9]+) tests run: ([0-9]+) passed.*?([0-9]+) skipped',nextest)
assert match and tuple(map(int,match.groups()))==(906,906,0),match.groups() if match else nextest[-1000:]
assert (m.OUT/'workspace-gate-7.log').read_text().find('ok')>=0
summary=json.loads((m.OUT/'runtime-summary.json').read_text());assert summary['source_commit']==m.SOURCE and summary['selected']==480
assert summary['strict_model_summary']=={'finite-model-match':59,'disagree':59}
guide=json.loads((m.OUT/'guides.json').read_text());assert guide['controls']==18 and guide['compiled_rust_controls']==6
for name,count,key in [('mutation-controls.json',39,'control_count'),('timeout-mutation-controls.json',60,'controls'),('admission/report.json',12,'controls'),('api-admission/report.json',12,'controls')]:
    data=json.loads((m.OUT/name).read_text());assert data[key]==count,(name,data[key])
tracked=subprocess.check_output(['git','ls-files','-z'],cwd=m.ROOT).decode().split('\0')
inputs={}
for name in tracked:
    if name and (name.endswith('.rs') or name=='.config/nextest.toml' or Path(name).name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','clippy.toml']):
        inputs[name]=m.sha(m.ROOT/name)
helpers={}
for name in ['dive-process-fairness-current-controls.py','dive-process-fairness-adversarial-controls.py','dive-record-process-fairness-aligned.py']:
    src=Path('/tmp')/name;dst=m.OUT/name;dst.write_bytes(src.read_bytes());helpers[name]=m.sha(dst)
evidence={str(p.relative_to(m.OUT)):m.sha(p) for p in m.OUT.rglob('*') if p.is_file() and p.name!='verification.json'}
runtime_inputs={}
for name in inputs:
    data=subprocess.check_output(['git','show',m.SOURCE+':'+name],cwd=m.ROOT)
    runtime_inputs[name]=hashlib.sha256(data).hexdigest()
changed=[name for name in inputs if inputs[name]!=runtime_inputs[name]]
assert sorted(changed)==['crates/dive-pluscal/tests/multiple_procedures.rs','crates/dive-pluscal/tests/process_procedures.rs'],changed
record={'schema_version':1,'source_commit':GATE_SOURCE,'runtime_source_commit':m.SOURCE,'runtime_source_sha256':runtime_inputs,'runtime_to_gate_equivalence':{'production_and_build_inputs_identical':True,'changed_rust_test_files':changed,'reason':'Two test-only updates reconcile14 newly proved fairness artifacts and move the same fair shared-procedure source from its stale refusal expectation to the existing positive oracle fixture. Failed903/906 producer gate retained separately.'},'family_certified':False,'scope':'Fresh bounded process-owned/shared-procedure translation and fairness artifact integration on shipped parallel checkpoint main. Source map is an integrity superset, not compiled source count. Strict namespace-pending witness reports retain actual exit1; canonical replay earns no strict TLC credit. No full translator/liveness/rewriting/checksum certificate.','native_artifacts':json.loads((m.ARTIFACTS/'manifest.json').read_text()),'source_sha256':inputs,'gate':{'rust_tests':906,'passed':906,'skipped':0,'logs':logs,'profile':'default','rustdoc_environment':'RUSTDOCFLAGS=-D warnings'},'runtime':summary,'guides':{'cli_controls':12,'compiled_rust_controls':6},'negative_controls':{'full_document_mutants':39,'timeout_row_mutants':60,'invalid_artifact_cli':12,'invalid_artifact_rust_api':12},'helpers_sha256':helpers,'evidence_sha256':evidence}
m.emit(m.OUT/'verification.json',record)
print(json.dumps({'source_commit':GATE_SOURCE,'runtime_source_commit':m.SOURCE,'integrity_inputs':len(inputs),'evidence_files':len(evidence),'tests':906,'selected':480,'artifacts':302,'finite_model_rows':118,'strict_pending_witnesses':59,'guide_controls':18}))
