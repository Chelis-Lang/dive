#!/usr/bin/env python3
"""Curated rewritten native artifacts versus clean primary finite checking."""
import argparse,collections,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
JAR='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
SELECTED=[('.', 'P_fresh'),('corrected-c','Corrected_C_fresh'),('fair-checksum','P_fair_initial_0'),('fair-checksum','C_fair_initial_0')]
def sha(data):return hashlib.sha256(data).hexdigest()
def counts(raw):
 found=re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue\.',raw);assert len(found)==1
 result=dict(zip(['generated','distinct','left_on_queue'],[int(x.replace(b',',b'')) for x in found[0]]));depth=re.findall(rb'The depth of the complete state graph search is ([0-9,]+)',raw)
 if depth:result['depth']=int(depth[0].replace(b',',b''))
 return result
def capture(command,work,out,name):
 p=subprocess.run(command,cwd=work,capture_output=True,timeout=30);saved={'exit_code':p.returncode}
 for stream in ['stdout','stderr']:
  data=getattr(p,stream);file=name+'.'+stream+'.gz';(out/file).write_bytes(gzip.compress(data,mtime=0));saved[stream]={'path':file,'sha256':sha(data)}
 return p,saved
def main(args):
 assert (BASE/'complete-full-controls/report.json').is_file(), 'complete Full report required before campaign'
 assert sha(args.jar.read_bytes())==JAR
 out=BASE/'finite-models';out.mkdir(exist_ok=True);results=[]
 refusal={'outcome':'retained-unchanged-baseline','evidence':'baseline/report.json'}
 for folder,name in [('primary',syntax+'_'+shape) for syntax in ['P','C'] for shape in ['constant','members','two-constant','weak','strong']]+[('owner-controls',syntax+'_'+shape) for syntax in ['P','C'] for shape in ['opposed-plus-minus','opposed-minus-plus','default-name']]+[('width-controls',syntax+'_'+shape) for syntax in ['P','C'] for shape in ['name-five','name-six','branch-long']]:
  directory=BASE/folder;source=(directory/(name+'.tla')).read_bytes().decode();primary=(directory/(name+'.translated.tla')).read_text();module=re.search(r'MODULE (\w+)',source).group(1)
  with tempfile.TemporaryDirectory(prefix='dive-full-model-generation-') as tmp:
   file=Path(tmp)/(module+'.tla');file.write_text(source);p,saved=capture([str(args.native),'translate',file.name,'--rewrite','--compat=tlc','--output','json'],Path(tmp),out,name+'.generation');doc=json.loads(p.stdout);assert p.returncode==0 and doc['accepted'] and doc['algorithm_checksum_verified'] is True and doc['model_proved'] is False;native=doc['replacement']['module_source']
  for witness in [False,True]:
   label=name+('.violation' if witness else '.complete');cfg='INIT Init\nNEXT Next\n'+('INVARIANT Good\n' if witness else '');suffix=('\nGood == g = 0 /\\ abcdef = 9\n' if 'branch-long' in name else '\nGood == g = 0\n') if witness else '';sources=[s.replace('====',suffix+'====') for s in [primary,native]]
   with tempfile.TemporaryDirectory(prefix='dive-rewritten-model-') as tmp:
    work=Path(tmp);file=work/(module+'.tla');config=work/(module+'.cfg');config.write_text(cfg);file.write_text(sources[0]);p,oracle=capture([str(args.java),'-Xmx256m','-cp',str(args.jar),'tlc2.TLC','-tool','-workers','1','-fp','0','-seed','0','-lncheck','final','-cleanup','-config',config.name,file.name],work,out,label+'.primary');assert p.returncode==(12 if witness else 0),(label,p.stdout,p.stderr);expected=counts(p.stdout);file.write_text(sources[1]);widths=[];validations=[];first=None
    for profile in ['dive','tlc']:
     for workers in [1,4,8]:
      for store in ['fp','exact']:
       p,saved=capture([str(args.native),'check',file.name,*(['--compat=tlc'] if profile=='tlc' else []),'--config',config.name,'--seed','0','--fp','0','--workers',str(workers),'--store',store,'--output','json'],work,out,label+f'.{profile}.w{workers}.{store}');actual=json.loads(p.stdout);assert p.returncode==((12 if profile=='tlc' else 10) if witness else 0) and actual['verdict']==('invariant-violation' if witness else 'success') and all(actual['statistics'][k]==v for k,v in expected.items()),(label,actual,expected)
       if first is None:first=actual
       if witness:assert actual['counterexample']['trace']==first['counterexample']['trace'],(label,actual)
       widths.append({'profile':profile,'workers':workers,'store':store,'outcome':'match','capture':saved})
    if witness:
     trace=first['counterexample']['trace']
     for control in ['original','invalid-init','invalid-next','missing-final-variable','invalid-closing']:
      candidate=json.loads(json.dumps(trace))
      if control=='invalid-init':candidate['states'][0]['values'].pop('g',None)
      elif control=='invalid-next':candidate['states'][-1]['values']['pc']={}
      elif control=='missing-final-variable':candidate['states'][-1]['values'].pop('g',None)
      elif control=='invalid-closing':candidate['states'].append(json.loads(json.dumps(candidate['states'][0])))
      (work/'trace.json').write_text(json.dumps(candidate));p,saved=capture([str(args.native),'validate-trace',file.name,'--config',config.name,'--trace','trace.json','--compat=tlc','--seed','0','--fp','0','--output','json'],work,out,label+'.'+control);actual=json.loads(p.stdout);assert (p.returncode==0 and actual['result']=='valid') or (p.returncode==3 and actual['result']=='unsupported') if control=='original' else (p.returncode!=0 and actual['result']!='valid'),(label,control,actual);validations.append({'control':control,'outcome':('valid' if p.returncode==0 else 'pending-named-refusal') if control=='original' else 'rejected','profile':'tlc','capture':saved})
   (out/(label+'.tla')).write_text(sources[1]);(out/(label+'.cfg')).write_text(cfg)
   results.append({'id':label,'outcome':'finite-model-match','primary_counts':expected,'primary':oracle,'source_sha256':sha(sources[1].encode()),'configuration_sha256':sha(cfg.encode()),'workers_stores':widths,'independent_validation':validations})
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'oracle_sha256':JAR,'native_sha256':sha(args.native.read_bytes()),'full_comparison_sha256':sha((BASE/'complete-full-controls/report.json').read_bytes()),'selected':len(results),'retained_generation_refusal':refusal,'valid_witness_controls':sum(sum(v['outcome']=='valid' for v in r['independent_validation']) for r in results),'rejected_witness_mutants':sum(sum(v['outcome']=='rejected' for v in r['independent_validation']) for r in results),'worker_store_controls':sum(len(r['workers_stores']) for r in results),'family_certified':False,'rows':results}
 (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Matched',len(results),'curated models;',report['worker_store_controls'],'worker/store controls;',report['valid_witness_controls'],'strict witnesses;',report['rejected_witness_mutants'],'mutants')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for n in ['java','jar','native']:p.add_argument('--'+n,type=Path,required=True)
 main(p.parse_args())
