"""Clean pinned final-only TLC, without trace-dump evaluation perturbation."""
import argparse,datetime,hashlib,json,re,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args();base=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=json.loads((base/'selection.json').read_text());out=base/'clean-tlc';out.mkdir();rows=[]
for case in selection:
 d=out/case['name'];d.mkdir();model=base/'models'/case['model']
 with tempfile.TemporaryDirectory(prefix='dive-temporal-if-clean-') as tmp:
  args=[a.java,'-cp',a.jar,'tlc2.TLC','-tool','-cleanup','-metadir',tmp,'-workers','1','-seed','0','-fp','0','-lncheck','final','-config',case['model']+'.cfg',case['model']+'.tla']
  try:r=subprocess.run(args,cwd=model,capture_output=True,timeout=30);code=r.returncode;stdout=r.stdout;stderr=r.stderr;timeout=False
  except subprocess.TimeoutExpired as e:code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
 (d/'stdout').write_bytes(stdout);(d/'stderr').write_bytes(stderr);codes=[int(c) for c in re.findall(rb'@!@!@STARTMSG (\d+):',stdout+stderr)]
 rows.append({'name':case['name'],'exit':code,'timeout':timeout,'message_codes':codes,'stdout_sha256':sha(d/'stdout'),'stderr_sha256':sha(d/'stderr'),'model_source_sha256':{str(p.relative_to(base)):sha(p) for p in model.iterdir() if p.suffix in ['.tla','.cfg']}})
(out/'capture.json').write_text(json.dumps({'schema_version':1,'oracle_sha256':sha(a.jar),'reason':a.reason,'timestamp':datetime.datetime.now(datetime.timezone.utc).isoformat(),'selection_sha256':sha(base/'selection.json'),'source_sha256':sha(Path(__file__)),'flags':{'workers':1,'seed':0,'fp':0,'lncheck':'final'},'rows':rows},indent=2)+'\n');print('Retained clean TLC selection',len(rows),'failures',sum(r['exit']not in [0,13] for r in rows))
