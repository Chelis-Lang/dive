from pathlib import Path
import hashlib,json,gzip,subprocess,tempfile,argparse,re
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--jar',required=True);a=p.parse_args();base=Path(__file__).resolve().parent;out=base/'primary';out.mkdir();pin='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d';assert hashlib.sha256(Path(a.jar).read_bytes()).hexdigest()==pin
cases=[('weak-over-strong','\\* PlusCal options (-wf)',['-sf']),('none-over-weak','\\* PlusCal options (-nof)',['-wf']),('first-declaration','\\* PlusCal options (-wf)\n\\* PlusCal options (-sf)',[]),('alias','\\* PlusCal options (wf)',[]),('labels','\\* PlusCal options (-label, -labelRoot Src)',[]),('contradictory','\\* PlusCal options (-wf, -sf)',[]),('unknown','\\* PlusCal options (-unknown)',[]),('unterminated','\\* PlusCal options (-wf',[])]
rows=[]
for syntax in ['C','P']:
 for name,comment,flags in cases:
  ident=syntax+'_'+name;module=ident.replace('-','_');body='variables g=0; process P=1 begin g:=1; end process; end algorithm;' if syntax=='P' else '{variables g=0; process(P=1){g:=1;}}';source=f'---- MODULE {module} ----\nEXTENDS Integers,TLC,Sequences\n{comment}\n(* --algorithm A {body} *)\n====\n';(out/(ident+'.tla')).write_text(source)
  with tempfile.TemporaryDirectory(prefix='dive-source-option-prestudy-') as tmp:
   file=Path(tmp)/(module+'.tla');file.write_bytes(source.encode());q=subprocess.run([a.java,'-cp',a.jar,'pcal.trans','-nocfg',*flags,str(file)],capture_output=True,timeout=30);artifact=file.read_bytes();(out/(ident+'.translated.tla')).write_bytes(artifact)
   for stream in ['stdout','stderr']:(out/(ident+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
   rows.append({'id':ident,'flags':flags,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'exit_code':q.returncode,'translated':b'BEGIN TRANSLATION' in artifact,'weak':b'WF_vars' in artifact,'strong':b'SF_vars' in artifact,'output_sha256':hashlib.sha256(artifact).hexdigest(),'stdout_sha256':hashlib.sha256(q.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(q.stderr).hexdigest()})
report={'schema_version':1,'oracle_sha256':pin,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'selected':len(rows),'native_credit':False,'rows':rows};(out/'manifest.json').write_text(json.dumps(report,indent=2)+'\n');print([(r['id'],r['exit_code'],r['translated'],r['weak'],r['strong']) for r in rows])
