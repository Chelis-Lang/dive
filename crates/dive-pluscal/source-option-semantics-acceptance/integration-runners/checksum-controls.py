from pathlib import Path
import subprocess,json,hashlib,gzip,re,tempfile
import argparse
p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--java',type=Path,required=True);p.add_argument('--jar',type=Path,required=True);p.add_argument('--adapter',type=Path,required=True);p.add_argument('--guides',type=Path,required=True);p.add_argument('--out',type=Path,required=True);args=p.parse_args()
O=args.out;O.mkdir(parents=True,exist_ok=False);JAVA=args.java;JAR=args.jar

def sha(x):return hashlib.sha256(x).hexdigest()
source=json.loads((args.guides/'P-tlc-rewrite.json').read_text())['replacement']['module_source']
controls=[('original',source,'NO_DIVERGENCE',True),('source',source.replace('L:g:=1','L:g:=2'),'PCAL_DIVERGENCE',False),('body',source.replace("/\\ g' = 1","/\\ g' = 2"),'TLA_DIVERGENCE',False),('pcal-header',source.replace('247b4187','00000000'),'PCAL_DIVERGENCE',False),('body-header',source.replace('989c1332','00000000'),'TLA_DIVERGENCE',False),('source-option',source.replace('(-wf)','(-sf)'),'PCAL_DIVERGENCE',False)]
rows=[]
for name,text,expected,accepted in controls:
 with tempfile.TemporaryDirectory(prefix='dive-source-options-crc-')as tmp:
  f=Path(tmp)/'SourceOptions.tla';f.write_text(text)
  p=subprocess.run([str(JAVA),'-cp',str(args.adapter)+':'+str(JAR),'pcal.OptionValidatorProbe','-sf',str(f)],capture_output=True,timeout=30)
  match=re.search(rb'VALIDATION\t([^\r\n]*)',p.stdout);assert p.returncode==0 and match;status=match[1].decode();assert expected in status,(name,status)
  q=subprocess.run([str(args.binary),'translate',str(f),'--rewrite','-sf','--compat=tlc','--output','json'],capture_output=True,timeout=30);d=json.loads(q.stdout);assert d['accepted']==accepted and d['model_proved']is False and d['exhaustive']is False and (accepted or'replacement'not in d),(name,d)
 for stem,raw in [('primary-stdout',p.stdout),('primary-stderr',p.stderr),('native-stdout',q.stdout),('native-stderr',q.stderr)]: (O/(name+'.'+stem+'.gz')).write_bytes(gzip.compress(raw,mtime=0))
 (O/(name+'.tla')).write_text(text);rows.append({'id':name,'outcome':'accepted-unchanged'if accepted else'rejected-corruption','validator':status,'native_exit':q.returncode,'source_sha256':sha(text.encode()),'native_sha256':sha(q.stdout)})
report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'native_binary_sha256':sha((args.binary).read_bytes()),'selected':len(rows),'model_proved':False,'family_certified':False,'rows':rows};(O/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('CRC controls:',len(rows),'passed',flush=True)
