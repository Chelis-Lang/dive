#!/usr/bin/env python3
"""Capture clean reference source-option spacing before extending guard claims."""
import argparse,gzip,hashlib,json,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
FORMS=[('tight-open','(*PlusCal options (-wf)*)'),('tight-line','\\*PlusCal options (-wf)'),('punctuation','(*;PlusCal options (-wf)*)'),('separated-punctuation','(*; PlusCal options (-wf)*)'),('tight-options','(* PlusCal options(-wf) *)'),('split','(* PlusCal\noptions (-wf) *)'),('tabs','(* PlusCal\toptions\t(-wf) *)'),('prefix','(* XPlusCal options (-wf) *)'),('missing-parenthesis','(* PlusCal options -wf *)'),('multiline-open','(*\nPlusCal options(-wf)\n*)')]
def sha(b):return hashlib.sha256(b).hexdigest()
def save(out,ident,p):
 for stream in ['stdout','stderr']:(out/(ident+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(p,stream),mtime=0))
 return {'exit_code':p.returncode,'stdout_sha256':sha(p.stdout),'stderr_sha256':sha(p.stderr)}
def main(a):
 assert sha(a.jar.read_bytes())==PIN;out=BASE/'source-spacing';out.mkdir();rows=[]
 for syntax in ['C','P']:
  for name,comment in FORMS:
   ident=syntax+'_'+name;module=ident.replace('-','_');algorithm='variables g=0; process P=1 begin L:g:=1; E:skip; end process; end algorithm;' if syntax=='P' else '{variables g=0; process(P=1){L:g:=1; E:skip;}}';source=f'---- MODULE {module} ----\nEXTENDS Integers,TLC,Sequences\n{comment}\n(* --algorithm A {algorithm} *)\n====\n';(out/(ident+'.tla')).write_bytes(source.encode())
   with tempfile.TemporaryDirectory(prefix='dive-source-option-spacing-') as tmp:
    file=Path(tmp)/(module+'.tla');file.write_bytes(source.encode());p=subprocess.run([str(a.java),'-cp',str(a.jar),'pcal.trans','-nocfg',str(file)],capture_output=True,timeout=30);primary=file.read_bytes();translated=b'BEGIN TRANSLATION' in primary;has_fairness=b'WF_vars' in primary;result={'id':ident,'source_sha256':sha(source.encode()),'primary':save(out,ident+'.primary',p),'primary_translated':translated,'primary_effective_weak_fairness':has_fairness};(out/(ident+'.translated.tla')).write_bytes(primary);file.write_bytes(source.encode());q=subprocess.run([str(a.native),'translate',str(file),'--rewrite','--compat=tlc','--output','json'],capture_output=True,timeout=30);d=json.loads(q.stdout);result['native']=save(out,ident+'.native',q);result['native_result']=d['result'];result['native_accepted']=d['accepted'];result['native_guarded']=q.returncode==3 and 'source option' in d.get('generation',d).get('diagnostic',{}).get('message','');assert not has_fairness or result['native_guarded'],result;rows.append(result)
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'oracle_sha256':PIN,'native_sha256':sha(a.native.read_bytes()),'family_certified':False,'selected':len(rows),'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print([(r['id'],r['primary_translated'],r['primary_effective_weak_fairness'],r['native_result']) for r in rows])
if __name__=='__main__':
 p=argparse.ArgumentParser()
 for n in ['java','jar','native']:p.add_argument('--'+n,type=Path,required=True)
 main(p.parse_args())
