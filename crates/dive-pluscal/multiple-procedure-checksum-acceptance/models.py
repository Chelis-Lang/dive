"""Declared finite disjoint procedure models, all widths/stores/profiles and independent mutants."""
from pathlib import Path
import argparse,gzip,hashlib,json,re,subprocess,tempfile
p=argparse.ArgumentParser()
for n in ['base','native','java','jar','out']:p.add_argument('--'+n,type=Path,required=True)
a=p.parse_args();a.out.mkdir(exist_ok=False)
def sha(b):return hashlib.sha256(b).hexdigest()
def run(argv,work,name):
 q=subprocess.run(list(map(str,argv)),cwd=work,capture_output=True,timeout=60);m={'exit_code':q.returncode}
 for s in ['stdout','stderr']:
  raw=getattr(q,s);path=name+'.'+s+'.gz';(a.out/path).write_bytes(gzip.compress(raw,mtime=0));m[s]={'path':path,'sha256':sha(raw)}
 return q,m
assert sha(a.jar.read_bytes())=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
manifest=json.loads((a.base/'primary/manifest.json').read_text())
selection_path=a.base/'model-selection.json'
selection=json.loads(selection_path.read_text())
assert selection['selected_before_execution']is True and len(selection['source_ids'])==len(set(selection['source_ids']))
by_id={r['id']:r for r in manifest['rows']}
assert all(ident in by_id and by_id[ident]['translated']and not by_id[ident]['reference_errors']and by_id[ident]['sany']['accepted']for ident in selection['source_ids'])
rows=[]
for entry in [by_id[ident]for ident in selection['source_ids']]:
  name=entry['id'];source=(a.base/'cases'/(name+'.tla')).read_text();primary=(a.base/'primary'/(name+'.translated.tla')).read_text()
  with tempfile.TemporaryDirectory(prefix='dive-macro-model-generation-')as tmp:
   work=Path(tmp);file=work/(name+'.tla');file.write_text(source);q,generated=run([a.native,'translate',file.name,'--rewrite','--compat=tlc','--output','json'],work,name+'.generation');doc=json.loads(q.stdout);assert q.returncode==0 and doc['accepted']and doc['algorithm_checksum_verified']is True and doc['model_proved']is False;native=doc['replacement']['module_source']
  for violation in [False,True]:
   ident=name+('.violation'if violation else'.complete');suffix='\nGood == pc # \"Done\"\n'if violation else '';config='INIT Init\nNEXT Next\n'+('INVARIANT Good\n'if violation else'');sources=[s.replace('====',suffix+'====')for s in [primary,native]]
   with tempfile.TemporaryDirectory(prefix='dive-macro-finite-model-')as tmp:
    work=Path(tmp);file=work/(name+'.tla');cfg=work/(name+'.cfg');cfg.write_text(config);file.write_text(sources[0]);q,reference=run([a.java,'-Xmx256m','-cp',a.jar,'tlc2.TLC','-tool','-workers','1','-fp','0','-seed','0','-lncheck','final','-cleanup','-config',cfg.name,file.name],work,ident+'.primary');assert q.returncode==(12 if violation else 0),(ident,q.stdout,q.stderr)
    match=re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue\.',q.stdout);assert len(match)==1;expected=dict(zip(['generated','distinct','left_on_queue'],[int(x.replace(b',',b''))for x in match[0]]));depth=re.findall(rb'The depth of the complete state graph search is ([0-9,]+)',q.stdout)
    if depth:expected['depth']=int(depth[0].replace(b',',b''))
    file.write_text(sources[1]);settings=[];validations=[];first=None
    for profile in ['dive','tlc']:
     for width in [1,4,8]:
      for store in ['fp','exact']:
       q,saved=run([a.native,'check',file.name,*(['--compat=tlc']if profile=='tlc'else[]),'--config',cfg.name,'--seed','0','--fp','0','--workers',str(width),'--store',store,'--output','json'],work,ident+f'.{profile}.w{width}.{store}');actual=json.loads(q.stdout);assert q.returncode==((12 if profile=='tlc'else 10)if violation else 0)and actual['verdict']==('invariant-violation'if violation else'success')and all(actual['statistics'][k]==v for k,v in expected.items()),(ident,expected,actual)
       if first is None:first=actual
       if violation:assert actual['counterexample']['trace']==first['counterexample']['trace']
       settings.append({'profile':profile,'workers':width,'store':store,'capture':saved,'outcome':'match'})
    if violation:
     trace=first['counterexample']['trace']
     for control in ['original','invalid-init','invalid-next','missing-final-variable','invalid-closing']:
      candidate=json.loads(json.dumps(trace))
      if control=='invalid-init':candidate['states'][0]['values'].pop('x',None)
      elif control=='invalid-next':candidate['states'][-1]['values']['x']=99
      elif control=='missing-final-variable':candidate['states'][-1]['values'].pop('x',None)
      elif control=='invalid-closing':candidate['states'].append(json.loads(json.dumps(candidate['states'][0])))
      (work/'trace.json').write_text(json.dumps(candidate));q,saved=run([a.native,'validate-trace',file.name,'--config',cfg.name,'--trace','trace.json','--compat=tlc','--seed','0','--fp','0','--output','json'],work,ident+'.'+control);actual=json.loads(q.stdout)
      if control=='original':assert(q.returncode==0 and actual['result']=='valid')or(q.returncode==3 and actual['result']=='unsupported');outcome='valid'if q.returncode==0 else'pending-named-refusal'
      else:assert q.returncode!=0 and actual['result']!='valid';outcome='rejected'
      validations.append({'control':control,'profile':'tlc','capture':saved,'outcome':outcome})
   (a.out/(ident+'.tla')).write_text(sources[1]);(a.out/(ident+'.cfg')).write_text(config);rows.append({'id':ident,'primary_counts':expected,'primary':reference,'generation':generated,'settings':settings,'independent_validation':validations,'outcome':'finite-model-match'});print(ident,'match',flush=True)
report={'schema_version':1,'selection_sha256':sha(selection_path.read_bytes()),'selected_source_ids':selection['source_ids'],'unselected_source_ids':[r['id']for r in manifest['rows']if r['id']not in selection['source_ids']],'selected':len(rows),'native_sha256':sha(a.native.read_bytes()),'oracle_sha256':sha(a.jar.read_bytes()),'script_sha256':sha(Path(__file__).read_bytes()),'worker_store_controls':sum(len(r['settings'])for r in rows),'valid_witness_controls':sum(sum(v['outcome']=='valid'for v in r['independent_validation'])for r in rows),'rejected_mutants':sum(sum(v['outcome']=='rejected'for v in r['independent_validation'])for r in rows),'family_certified':False,'rows':rows};(a.out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('complete',len(rows),flush=True)
