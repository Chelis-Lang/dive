#!/usr/bin/env python3
"""Retain primary configured/unconfigured validation, never infer source identity."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(b):return hashlib.sha256(b).hexdigest()
def save(out,ident,p):
 for stream in ['stdout','stderr']:(out/(ident+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(p,stream),mtime=0))
 return {'exit_code':p.returncode,'stdout_sha256':sha(p.stdout),'stderr_sha256':sha(p.stderr)}
def main(a):
 assert sha(a.jar.read_bytes())==PIN;out=BASE/'validator-context';out.mkdir();rows=[];manifest=json.loads((BASE/'primary/manifest.json').read_text())
 for ident in ['P_plain_weak','P_plain_strong','P_missing_label-root','P_modifiers_none','P_fair_missing_label','C_loop_label']:
  row=next(r for r in manifest['rows'] if r['id']==ident);assert row['primary_translated'];original=(BASE/'primary'/(ident+'.translated.tla')).read_bytes();module=re.search(rb'MODULE (\w+)',original)[1].decode()
  with tempfile.TemporaryDirectory(prefix='dive-primary-validator-') as tmp:
   file=Path(tmp)/(module+'.tla');file.write_bytes(original);result={'id':ident,'primary_module_sha256':sha(original),'flags':row['flags']}
   for mode,flags in [('unconfigured',[]),('configured',row['flags'])]:
    p=subprocess.run([str(a.java),'-cp',str(a.adapter)+':'+str(a.jar),'pcal.OptionValidatorProbe','-nocfg',*flags,str(file)],capture_output=True,timeout=30);assert p.returncode==0;result[mode]=save(out,ident+'.'+mode,p);result[mode]['validation']=re.findall(rb'VALIDATION\t(.*)',p.stdout)[0].decode();assert file.read_bytes()==original
   rows.append(result)
 for name,flags in [('weak',['-wf']),('strong',['-sf']),('labels',['-label']),('root',['-label','-labelRoot','X_'])]:
  ident='SourceOptions_'+name;body='g:=1; g:=2;' if name in ['labels','root'] else 'L:g:=1; E:skip;';source=f'---- MODULE {ident} ----\nEXTENDS Integers,TLC,Sequences\n\\* PlusCal options ({", ".join(flags)})\n(* --algorithm A variables g=0; process P \\in {{1,2}} begin {body} end process; end algorithm; *)\n====\n'
  with tempfile.TemporaryDirectory(prefix='dive-source-options-') as tmp:
   file=Path(tmp)/(ident+'.tla');file.write_bytes(source.encode());p=subprocess.run([str(a.java),'-cp',str(a.jar),'pcal.trans','-nocfg',str(file)],capture_output=True,timeout=30);primary=file.read_bytes();accepted=b'BEGIN TRANSLATION' in primary;result={'id':ident,'source_options':flags,'primary':save(out,ident+'.primary',p),'primary_translated':accepted,'source_sha256':sha(source.encode())};(out/(ident+'.tla')).write_bytes(source.encode());(out/(ident+'.translated.tla')).write_bytes(primary)
   q=subprocess.run([str(a.java),'-cp',str(a.adapter)+':'+str(a.jar),'pcal.OptionValidatorProbe','-nocfg',str(file)],capture_output=True,timeout=30);assert q.returncode==0;result['unconfigured']=save(out,ident+'.unconfigured',q);result['unconfigured']['validation']=re.findall(rb'VALIDATION\t(.*)',q.stdout)[0].decode();file.write_bytes(source.encode())
   q=subprocess.run([str(a.native),'translate',str(file),'--rewrite','--compat=tlc','--output','json'],capture_output=True,timeout=30);result['before_source_option_guard']=save(out,ident+'.before-native-guard',q);doc=json.loads(q.stdout);result['native_before_result']=doc['result'];result['native_before_accepted']=doc['accepted'];rows.append(result)
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'oracle_sha256':PIN,'validator_sha256':sha((a.adapter/'pcal/OptionValidatorProbe.class').read_bytes()),'native_before_sha256':sha(a.native.read_bytes()),'family_certified':False,'selected':len(rows),'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print([(r['id'],r.get('unconfigured',{}).get('validation'),r.get('configured',{}).get('validation'),r.get('native_before_result')) for r in rows])
if __name__=='__main__':
 p=argparse.ArgumentParser()
 for n in ['java','jar','adapter','native']:p.add_argument('--'+n,type=Path,required=True)
 main(p.parse_args())
