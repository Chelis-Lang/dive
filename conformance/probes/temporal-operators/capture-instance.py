"""Preserve every clean pinned material-control outcome, including errors/timeouts."""
from pathlib import Path
import argparse,hashlib,json,subprocess,tempfile,time,re,datetime
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--jar',required=True);p.add_argument('--output',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
base=Path(__file__).resolve().parent;output=Path(a.output);assert not output.exists(),'preserve previous captures'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=json.loads((base/'selection-instance.json').read_text());output.mkdir(parents=True);rows=[]
for row in selection:
 name=row['name'];folder=base/'models'/name;raw=output/name;raw.mkdir();start=time.monotonic()
 with tempfile.TemporaryDirectory(prefix='dive-material-primary-')as metadir:
  cmd=[a.java,'-cp',a.jar,'tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-metadir',metadir,name]
  try:
   c=subprocess.run(cmd,cwd=folder,capture_output=True,timeout=30);stdout=c.stdout;stderr=c.stderr;code=c.returncode;timeout=False
  except subprocess.TimeoutExpired as e:stdout=e.stdout or b'';stderr=e.stderr or b'';code=None;timeout=True
 (raw/'stdout').write_bytes(stdout);(raw/'stderr').write_bytes(stderr)
 rows.append(dict(name=name,kind=row['kind'],exit=code,timeout=timeout,elapsed_seconds=time.monotonic()-start,message_codes=[int(x)for x in re.findall(rb'STARTMSG (\d+):',stdout)],model_source_sha256={str(p.relative_to(base)):sha(p)for p in folder.iterdir()if p.suffix in ['.tla','.cfg']},stdout_sha256=sha(raw/'stdout'),stderr_sha256=sha(raw/'stderr')))
record=dict(schema_version=1,oracle='tlc-2026.10.06.014338',oracle_sha256=sha(a.jar),reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),flags=dict(workers=1,seed=0,fp=0,lncheck='final'),selection_sha256=sha(base/'selection-instance.json'),capture_source_sha256=sha(__file__),rows=rows)
(output/'capture.json').write_text(json.dumps(record,indent=2)+'\n');print('Preserved',len(rows),'rows;',sum(r['timeout']for r in rows),'timeouts; exits',[(r['name'],r['exit'])for r in rows])
