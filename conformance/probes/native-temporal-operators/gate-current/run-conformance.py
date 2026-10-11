from pathlib import Path
import os,json,hashlib,subprocess,time
r=Path.cwd();b=r/'conformance/probes/native-temporal-operators/gate-current';f=Path('/tmp/dive-native-operator-frozen-native');sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources=json.loads((b/'before.json').read_text())['source_sha256'];binary={n:sha(f/n)for n in ['dive','dive-conformance']};before=dict(source_sha256=sources,binaries_sha256=binary);(b/'conformance-before.json').write_text(json.dumps(before,indent=2)+'\n');env=os.environ.copy();env['DIVE_CACHE_DIR']='/Users/jeff/code/dive-native-operator-cache';steps=[]
for corpus,filename,count in [('examples','cm1.txt',21),('validation-smoke','validation-smoke.txt',16)]:
 filters=[x.split('#')[0].strip()for x in (r/'conformance/gates'/filename).read_text().splitlines()if x.split('#')[0].strip()];assert len(filters)==count
 reports=b/(corpus+'-reports');cmd=[str(f/'dive-conformance'),'run','--impl','dive','--dive-bin',str(f/'dive'),'--impl-profile','tlc','--corpus',corpus,'--tier','all','--no-sany','--no-summary','--workers','1,4,8','--jobs','1','--report-dir',str(reports)]
 for item in filters:cmd+=['--filter',item]
 start=time.monotonic()
 with (b/(corpus+'-gate.log')).open('wb')as out:c=subprocess.run(cmd,cwd=r,env=env,stdout=out,stderr=subprocess.STDOUT)
 report=json.loads(next(reports.glob('*.json')).read_text());summary=report['summary']['models'];assert c.returncode==0 and summary['agree']==count and all(x['outcome']=='agree'for x in report['models']),summary
 steps.append(dict(corpus=corpus,model_count=count,workers=[1,4,8],exit=c.returncode,wall_seconds=time.monotonic()-start,command=cmd,summary=summary));print(corpus,count,'agree',flush=True)
assert all(sha(r/p)==h for p,h in sources.items()) and all(sha(f/n)==h for n,h in binary.items())
(b/'conformance-after.json').write_text(json.dumps(dict(source_and_binaries_unchanged=True,source_sha256=sources,binaries_sha256=binary,steps=steps,artifact_sha256={str(p.relative_to(b)):sha(p)for p in b.rglob('*')if p.is_file()and ('reports' in str(p)or p.name in ['examples-gate.log','validation-smoke-gate.log','conformance-before.json'])}),indent=2)+'\n')
