"""Separate fresh direct-Explode ownership study; never primary header CRC authority."""
from pathlib import Path
import argparse,base64,gzip,hashlib,json,subprocess,tempfile
sha=lambda b:hashlib.sha256(b).hexdigest()
p=argparse.ArgumentParser(description=__doc__)
for name in ['java','jar','adapter','library','out']:p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();B=Path(__file__).resolve().parent;selection=json.loads((B/'selection.json').read_bytes());assert sha(a.jar.read_bytes())=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d';a.out.mkdir(exist_ok=False);rows=[]
for row in selection['rows']:
 source=(B/'cases'/(row['id']+'.tla')).read_bytes();assert sha(source)==row['input_sha256']
 with tempfile.TemporaryDirectory(prefix='dive-nested-proc-explode-')as tmp:
  work=Path(tmp);file=work/(row['id']+'.tla');file.write_bytes(source)
  q=subprocess.run([str(a.java),'-Xmx256m','-DTLA-Library='+str(a.library),'-cp',str(a.adapter)+':'+str(a.jar),'pcal.ProcedureExplodeProbe',*row['flags'],file.name],cwd=work,capture_output=True,timeout=30)
  record={'id':row['id'],'input_sha256':sha(source),'exit_code':q.returncode,'stages':{},'rejections':[]}
  for stream in ['stdout','stderr']:
   raw=getattr(q,stream);name=row['id']+'.'+stream+'.gz';(a.out/name).write_bytes(gzip.compress(raw,mtime=0));record[stream]={'path':name,'sha256':sha(raw)}
  for line in q.stdout.decode().splitlines():
   fields=line.split('\t')
   if fields[0]=='STAGE'and len(fields)==5:
    raw=base64.b64decode(fields[3]);doc=base64.b64decode(fields[4]);(a.out/(row['id']+'.'+fields[1]+'.ast')).write_bytes(raw);(a.out/(row['id']+'.'+fields[1]+'.json')).write_bytes(doc);record['stages'][fields[1]]={'crc':fields[2],'ast_sha256':sha(raw),'json_sha256':sha(doc)}
   elif fields[0]=='REJECTED':record['rejections'].append(base64.b64decode(fields[1]).decode())
  rows.append(record)
 if len(rows)%20==0:print('captured',len(rows),flush=True)
report={'schema_version':1,'selected':len(rows),'oracle_sha256':sha(a.jar.read_bytes()),'adapter_sha256':sha((a.adapter/'pcal/ProcedureExplodeProbe.class').read_bytes()),'script_sha256':sha(Path(__file__).read_bytes()),'selection_sha256':sha((B/'selection.json').read_bytes()),'header_crc_authority':False,'native_credit':False,'family_certified':False,'rows':rows};(a.out/'manifest.json').write_text(json.dumps(report,indent=2)+'\n');print('complete',len(rows),flush=True)
