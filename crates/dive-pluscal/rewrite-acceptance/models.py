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
 assert sha(args.jar.read_bytes())==JAR
 comparison=json.loads((BASE/'comparison.json').read_text());out=BASE/'finite-models';out.mkdir(exist_ok=True);results=[]
 for campaign,name in SELECTED:
  row=next(r for r in comparison['rows'] if r['id']==name);directory=BASE/campaign
  doc=json.loads(gzip.decompress((directory/'native'/row['tlc']['capture']['stdout']).read_bytes()));native=doc['replacement']['module_source'];m=json.loads((directory/'primary/manifest.json').read_text());primaryrow=next(r for r in m['rows'] if r['id']==name);primary=(directory/'primary'/primaryrow['translated']['path']).read_text();module=re.search(r'MODULE (\w+)',native).group(1)
  for witness in [False,True]:
   label=name+('.violation' if witness else '.complete');cfg='INIT Init\nNEXT Next\n'+('INVARIANT Good\n' if witness else '');suffix='\nGood == pc # "E"\n' if witness else '';sources=[s.replace('====',suffix+'====') for s in [primary,native]]
   with tempfile.TemporaryDirectory(prefix='dive-rewritten-model-') as tmp:
    work=Path(tmp);file=work/(module+'.tla');config=work/(module+'.cfg');config.write_text(cfg);file.write_text(sources[0]);p,oracle=capture([str(args.java),'-Xmx256m','-cp',str(args.jar),'tlc2.TLC','-tool','-workers','1','-fp','0','-seed','0','-lncheck','final','-cleanup','-config',config.name,file.name],work,out,label+'.primary');assert p.returncode==(12 if witness else 0),(label,p.stdout,p.stderr);expected=counts(p.stdout);file.write_text(sources[1]);widths=[];validations=[];first=None
    for workers in [1,4,8]:
     for store in ['fp','exact']:
      p,saved=capture([str(args.native),'check',file.name,'--compat=tlc','--config',config.name,'--seed','0','--fp','0','--workers',str(workers),'--store',store,'--output','json'],work,out,label+f'.w{workers}.{store}');actual=json.loads(p.stdout);assert p.returncode==(12 if witness else 0) and actual['verdict']==('invariant-violation' if witness else 'success') and all(actual['statistics'][k]==v for k,v in expected.items()),(label,actual,expected)
      if first is None:first=actual
      if witness:assert actual['counterexample']['trace']==first['counterexample']['trace']
      widths.append({'workers':workers,'store':store,'outcome':'match','capture':saved})
    if witness:
     trace=first['counterexample']['trace']
     for control in ['original','invalid-init','invalid-next','missing-final-variable','invalid-closing']:
      candidate=json.loads(json.dumps(trace))
      if control=='invalid-init':candidate['states'][0]['values'].pop('x',None)
      elif control=='invalid-next':candidate['states'][-1]['values']['pc']={}
      elif control=='missing-final-variable':candidate['states'][-1]['values'].pop('x',None)
      elif control=='invalid-closing':candidate['states'].append(json.loads(json.dumps(candidate['states'][0])))
      (work/'trace.json').write_text(json.dumps(candidate));p,saved=capture([str(args.native),'validate-trace',file.name,'--config',config.name,'--trace','trace.json','--compat=tlc','--seed','0','--fp','0','--output','json'],work,out,label+'.'+control);actual=json.loads(p.stdout);assert (p.returncode==0 and actual['result']=='valid') if control=='original' else (p.returncode!=0 and actual['result']!='valid'),(label,control,actual);validations.append({'control':control,'outcome':'valid' if control=='original' else 'rejected','capture':saved})
   (out/(label+'.tla')).write_text(sources[1]);(out/(label+'.cfg')).write_text(cfg)
   results.append({'id':label,'outcome':'finite-model-match','primary_counts':expected,'primary':oracle,'source_sha256':sha(sources[1].encode()),'configuration_sha256':sha(cfg.encode()),'workers_stores':widths,'independent_validation':validations})
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'oracle_sha256':JAR,'native_sha256':sha(args.native.read_bytes()),'comparison_sha256':sha((BASE/'comparison.json').read_bytes()),'selected':len(results),'worker_store_controls':sum(len(r['workers_stores']) for r in results),'family_certified':False,'rows':results}
 (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Matched',len(results),'curated models;',report['worker_store_controls'],'worker/store controls; four strict witnesses and sixteen mutants')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for n in ['java','jar','native']:p.add_argument('--'+n,type=Path,required=True)
 main(p.parse_args())
