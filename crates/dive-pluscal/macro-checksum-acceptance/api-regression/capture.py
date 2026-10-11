"""Clean primary macro translation and exact development AST-stage captures."""
from pathlib import Path
import argparse,base64,gzip,hashlib,json,re,subprocess,tempfile
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(data):return hashlib.sha256(data).hexdigest()
p=argparse.ArgumentParser(description=__doc__)
for name in ['java','jar','adapter','out','library']:p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();assert sha(a.jar.read_bytes())==PIN
base=Path(__file__).resolve().parent;selection=json.loads((base/'selection.json').read_text());a.out.mkdir(exist_ok=False)
def run(argv,work,stem):
 q=subprocess.run([str(a.java),'-Xmx256m','-DTLA-Library='+str(a.library),'-cp',str(a.adapter)+':'+str(a.jar),*argv],cwd=work,capture_output=True,timeout=30)
 metadata={'exit_code':q.returncode}
 for stream in ['stdout','stderr']:
  raw=getattr(q,stream);name=stem+'.'+stream+'.gz';(a.out/name).write_bytes(gzip.compress(raw,mtime=0));metadata[stream]={'path':name,'sha256':sha(raw)}
 return q,metadata
rows=[]
for index,r in enumerate(selection['rows']):
 name=r['id'];source=(base/'cases'/(name+'.tla')).read_bytes();assert sha(source)==r['input_sha256'];(a.out/(name+'.tla')).write_bytes(source)
 with tempfile.TemporaryDirectory(prefix='dive-macro-full-primary-')as tmp:
  work=Path(tmp);file=work/(re.search(rb'MODULE\s+([A-Za-z][A-Za-z0-9_]*)',source).group(1).decode()+'.tla');file.write_bytes(source)
  q,primary=run(['pcal.trans','-nocfg',*r['flags'],file.name],work,name+'.primary');artifact=file.read_bytes();(a.out/(name+'.translated.tla')).write_bytes(artifact)
  error=b'Unrecoverable error' in q.stdout+q.stderr;translated=b'BEGIN TRANSLATION' in artifact;header=re.search(rb'chksum\(pcal\)\s*=\s*"([0-9a-f]+)"\s*/\\\s*chksum\(tla\)\s*=\s*"([0-9a-f]+)"',artifact)
  file.write_bytes(source);stage,stagemeta=run(['pcal.SourceOptionProbe',*r['flags'],file.name],work,name+'.stages');stages={};context=None
  for line in stage.stdout.decode().splitlines():
   fields=line.split('\t')
   if fields[0]=='SOURCE_CONTEXT'and len(fields)==4:context={'status':int(fields[1]),'found':fields[2]=='true','settings':json.loads(base64.b64decode(fields[3]))}
   if fields[0]=='STAGE'and len(fields)==5:
    raw=base64.b64decode(fields[3]);doc=base64.b64decode(fields[4]);(a.out/(name+'.'+fields[1]+'.ast')).write_bytes(raw);(a.out/(name+'.'+fields[1]+'.json')).write_bytes(doc);stages[fields[1]]={'crc':fields[2],'ast_sha256':sha(raw),'json_sha256':sha(doc)}
  row={**r,'output_sha256':sha(artifact),'primary':primary,'reference_errors':error,'translated':translated,'header':None if header is None else[s.decode()for s in header.groups()],'stages_process':stagemeta,'context':context,'stages':stages}
  if translated and not error:
   assert header is not None and 'translated'in stages and stages['translated']['crc']==header[1].decode(),row
   file.write_bytes(artifact);sany,saved=run(['tla2sany.SANY','-error-codes',file.name],work,name+'.sany');row['sany']={**saved,'accepted':sany.returncode==0}
   validator,saved=run(['pcal.OptionValidatorProbe',*r['flags'],file.name],work,name+'.validator');row['validator']={**saved,'stdout':validator.stdout.decode()}
   assert validator.returncode==0, row
  rows.append(row)
 if len(rows)%20==0:print('captured',len(rows),flush=True)
report={'schema_version':1,'oracle_sha256':PIN,'oracle_commit':'94d0c5070852b37b801bb8998c436383ab7ee37b','reference_library_sha256':{str(p.relative_to(a.library)):sha(p.read_bytes())for p in sorted(a.library.glob('*.tla'))},'selection_sha256':sha((base/'selection.json').read_bytes()),'capture_sha256':sha(Path(__file__).read_bytes()),'adapter_sha256':{n:sha((a.adapter/'pcal'/n).read_bytes())for n in ['SourceOptionProbe.class','OptionValidatorProbe.class']},'selected':len(rows),'native_credit':False,'family_certified':False,'rows':rows}
(a.out/'manifest.json').write_text(json.dumps(report,indent=2)+'\n');print('complete',len(rows),flush=True)
