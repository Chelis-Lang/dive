from pathlib import Path
import json,subprocess,gzip,hashlib
import argparse
p=argparse.ArgumentParser()
for key in ['base','runtime','java','jar','adapter','library','out']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();b=a.runtime;out=a.out;out.mkdir(exist_ok=False);script=a.base/'public-controls.py';runner=out/'compare.py';runner.write_bytes(script.read_bytes());guide=a.base/'guide';rows=[];java=a.java;jar=a.jar
for field,value in [('baseline',None),('model_proved',True),('exhaustive',True),('result','internal-error'),('profile','invented'),('algorithm_checksum_verified',False),('exit_code',3)]:
 folder=out/field;folder.mkdir();documents={}
 for syntax in ['C','P']:
  name=f'ProcedureChecksum_{syntax}'
  for profile in ['dive','tlc']:
   raw=gzip.decompress((b/'guides-final'/f'{name}.{profile}.cli-full.stdout.gz').read_bytes());doc=json.loads(raw)
   if field!='baseline':doc[field]=value
   documents[name+'.'+profile]=doc
 (folder/'forged.json').write_text(json.dumps(documents));fake=folder/'fake.py';fake.write_text('#!/usr/bin/env python3\nimport json,sys\nfrom pathlib import Path\nkey=Path(sys.argv[2]).stem+"."+("tlc"if"--compat=tlc"in sys.argv else"dive");d=json.loads(Path(__file__).with_name("forged.json").read_text())[key];print(json.dumps(d));raise SystemExit(d["exit_code"])\n');fake.chmod(0o755)
 q=subprocess.run(['python3','-I',str(runner),'--base',str(guide),'--binary',str(fake),'--cst',str(b/'bins/cst'),'--java',str(java),'--jar',str(jar),'--adapter',str(a.adapter),'--library',str(a.library),'--out',str(folder/'outputs')],capture_output=True,timeout=60);(folder/'run.stdout').write_bytes(q.stdout);(folder/'run.stderr').write_bytes(q.stderr);report=json.loads((folder/'outputs/report.json').read_text());expected={'admitted-exact-cst-sany-validator':4}if field=='baseline'else{'disagree':4};assert q.returncode==int(field!='baseline')and report['summary']==expected,(field,q.returncode,q.stdout,q.stderr);rows.append({'mutation':field,'controls':4,'exit_code':q.returncode,'outcome':'genuine-reference-accepted'if field=='baseline'else'all-forgeries-rejected'})
(out/'report.json').write_text(json.dumps({'schema_version':1,'selected':28,'genuine_controls':4,'forged_controls':24,'rows':rows,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'strict_original_script_sha256':hashlib.sha256(script.read_bytes()).hexdigest(),'derived_single-guide_folder_script_sha256':hashlib.sha256(runner.read_bytes()).hexdigest(),'family_certified':False},indent=2)+'\n');print('Accepted4genuine and rejected24matched-source full-schema forged publicreports')
