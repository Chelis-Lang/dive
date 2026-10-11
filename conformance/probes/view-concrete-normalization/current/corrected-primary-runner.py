from pathlib import Path
import hashlib,json,subprocess,shutil,re
r=Path('/Users/jeff/code/dive-view-normalization-work');a=Path('/Users/jeff/code/dive-view-normalization-artifacts/current-corrected-primary');a.mkdir(exist_ok=False);cache=Path('/Users/jeff/code/dive-view-random-stream-cache');java=next(cache.glob('jre/**/bin/java'));jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar';sha=lambda b:hashlib.sha256(b).hexdigest();assert sha(jar.read_bytes())=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d';rows=[]
for name in ['reverse-mixed','mixed-set','set','tuple-set','record-set','function-set']:
 p=a/name;p.mkdir();source=r/'conformance/probes/view-concrete-normalization/before'/name
 for f in ['Compound.tla','Compound.cfg']:shutil.copyfile(source/f,p/f)
 cmd=[str(java),'-Xmx512m','-cp',str(jar),'tlc2.TLC','-tool','-workers','1','-seed','7','-fp','0','-cleanup','-metadir',str(p/'meta'),'Compound'];v=subprocess.run(cmd,cwd=p,capture_output=True,timeout=30);(p/'stdout').write_bytes(v.stdout);(p/'stderr').write_bytes(v.stderr)
 codes=[int(n)for n in re.findall(rb'STARTMSG (\d+):',v.stdout)];stats=re.search(rb'(\d+) states generated, (\d+) distinct states found',v.stdout);depth=re.search(rb'The depth of the complete state graph search is (\d+)',v.stdout)
 row=dict(model=name,exit=v.returncode,codes=codes,states=v.stdout.count(b'STARTMSG 2217:4'),generated=int(stats[1])if stats else None,distinct=int(stats[2])if stats else None,depth=int(depth[1])if depth else None,files_sha256={f:sha((p/f).read_bytes())for f in['Compound.tla','Compound.cfg','stdout','stderr']});rows.append(row);print(name,v.returncode,row['generated'],row['distinct'],row['depth'],flush=True)
 assert v.returncode==(255 if name=='reverse-mixed' else 12),row
 if name!='reverse-mixed':assert(row['generated'],row['distinct'],row['depth'],row['states'])==(4,4,4,4),row
(a/'capture.json').write_text(json.dumps(dict(oracle_source_commit='94d0c5070852b37b801bb8998c436383ab7ee37b',oracle_sha256=sha(jar.read_bytes()),java_sha256=sha(java.read_bytes()),flags=['-tool','-workers','1','-seed','7','-fp','0','-cleanup','-metadir','<private>'],reason='Fresh clean primary recapture for concrete VIEW normalization repair; no trace instrumentation',rows=rows),indent=2)+'\n')
