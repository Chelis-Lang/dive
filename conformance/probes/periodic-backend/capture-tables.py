"""Closed actual pointer-table/snapshot projection; retain compiler and process failures."""
import argparse,datetime,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--javac',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
r=Path(__file__).resolve().parent; sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
raw=r/'tables-raw';raw.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='dive-periodic-tables-') as t:
 c=subprocess.run([a.javac,'-cp',a.jar,'-d',t,str(r/'TableProbe.java')],capture_output=True)
 (raw/'compiler.stdout').write_bytes(c.stdout);(raw/'compiler.stderr').write_bytes(c.stderr)
 rows=[]
 if c.returncode==0:
  try:q=subprocess.run([a.java,'-ea','-cp',t+':'+a.jar,'tlc2.tool.liveness.TableProbe',t],capture_output=True,timeout=20);code=q.returncode;out=q.stdout;err=q.stderr;timed=False
  except subprocess.TimeoutExpired as e:code=None;out=e.stdout or b'';err=e.stderr or b'';timed=True
  (raw/'stdout').write_bytes(out);(raw/'stderr').write_bytes(err)
  fields={}
  for line in out.decode().splitlines():
   if '\t' in line:
    k,v=line.split('\t',1);assert k not in fields;fields[k]=v
  rows.append(dict(exit=code,timeout=timed,fields=fields))
 record=dict(schema_version=1,scope='actual DiskGraph/TableauDiskGraph pointer count, restoration and snapshot projection; no model proof',oracle='tlc-2026.10.06.014338',oracle_sha256=sha(a.jar),reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_sha256={f:sha(r/f) for f in ['TableProbe.java','capture-tables.py']},compiler_exit=c.returncode,rows=rows,artifacts={str(p.relative_to(r)):sha(p) for p in raw.iterdir() if p.is_file()})
 (r/'capture-tables.json').write_text(json.dumps(record,indent=2)+'\n');print('compiler',c.returncode,'rows',len(rows),'exits',[q['exit'] for q in rows])
 if c.returncode:raise SystemExit(c.returncode)
