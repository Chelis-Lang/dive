from pathlib import Path
import subprocess,os,json,time,hashlib
r=Path('/Users/jeff/code/dive-final-scopes-work');out=r/'conformance/probes/tlc-final-scopes/gate-current';cli=Path('/tmp/dive-final-scopes-frozen-native/dive');harness=Path('/Users/jeff/code/dive-final-scopes-target/debug/dive-conformance');env=dict(os.environ,DIVE_CACHE_DIR='/Users/jeff/code/dive-final-scopes-cache')
rows=[]
for corpus,listfile in [('examples','conformance/gates/cm1.txt'),('validation-smoke','conformance/gates/validation-smoke.txt')]:
 selection=[l.split('#',1)[0].strip()for l in (r/listfile).read_text().splitlines()];selection=[s for s in selection if s]
 reports=out/(corpus+'-reports');reports.mkdir(exist_ok=True)
 cmd=[str(harness),'run','--impl','dive','--impl-profile','tlc','--corpus',corpus,'--tier','all','--no-sany','--no-summary','--report-dir',str(reports),'--workers','1,4,8','--dive-bin',str(cli)]
 for s in selection:cmd+=['--filter',s]
 print('Running conformance',corpus,len(selection),'models, widths1/4/8',flush=True);start=time.monotonic()
 with (out/(corpus+'-gate.log')).open('wb')as log:run=subprocess.run(cmd,cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT)
 text=(out/(corpus+'-gate.log')).read_text();status=[s for s in text.splitlines()if s.startswith('[')and '/'+str(len(selection))+'] 'in s]
 assert run.returncode==0,(corpus,run.returncode,text[-1000:]);assert len(status)==len(selection),(corpus,len(status));assert all('] agree 'in s for s in status),status
 rows.append(dict(corpus=corpus,model_count=len(selection),workers=[1,4,8],exit=run.returncode,wall_seconds=time.monotonic()-start));print('Passed',corpus,flush=True)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();before=json.loads((out/'conformance-before.json').read_text());assert sha(cli)==before['cli_sha256']and sha(harness)==before['harness_sha256']
 rust=json.loads((out/'before.json').read_text());assert all(sha(r/p)==d for p,d in rust['source_sha256'].items())
 (out/'conformance-after.json').write_text(json.dumps(dict(source_and_binaries_unchanged=True,rows=rows,artifact_sha256={str(p.relative_to(out)):sha(p)for corpus in ['examples','validation-smoke']for p in [out/(corpus+'-gate.log'),*sorted((out/(corpus+'-reports')).glob('*.json'))]}),indent=2)+'\n')
