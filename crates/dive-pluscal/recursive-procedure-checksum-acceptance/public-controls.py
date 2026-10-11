"""Complete stage-separated public procedure Full admission; known mismatches stay red."""
from pathlib import Path
import argparse,collections,gzip,hashlib,json,re,subprocess,tempfile
p=argparse.ArgumentParser();
for key in ['base','binary','cst','java','jar','adapter','library','out']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();a.out.mkdir(exist_ok=False)
def sha(b):return hashlib.sha256(b).hexdigest()
def run(command):return subprocess.run(list(map(str,command)),capture_output=True,timeout=30)
def java(args):return run([a.java,'-Xmx256m','-DTLA-Library='+str(a.library),'-cp',str(a.adapter)+':'+str(a.jar),*args])
def save(ident,stem,q):
 for stream in ['stdout','stderr']:(a.out/(ident+'.'+stem+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
 return {'exit_code':q.returncode,'stdout_sha256':sha(q.stdout),'stderr_sha256':sha(q.stderr)}
assert sha(a.jar.read_bytes())=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
rows=[]
for folder in ['', 'tail-ownership']:
 b=a.base/folder;manifest=json.loads((b/'primary/manifest.json').read_text())
 for r in manifest['rows']:
  source=b/'cases'/(r['id']+'.tla');module=re.search(rb'MODULE\s+([A-Za-z][A-Za-z0-9_]*)',source.read_bytes()).group(1).decode()
  for profile in ['dive','tlc']:
   ident=(folder.replace('../','').replace('/','_')+'_'+r['id']+'_'+profile).strip('_');detail='';statuses={};outcome='disagree'
   with tempfile.TemporaryDirectory(prefix='dive-macro-full-public-')as tmp:
    work=Path(tmp);file=work/(module+'.tla');file.write_bytes(source.read_bytes());q=run([a.binary,'translate',file,'--rewrite','--replace-modified','--output','json',*r['flags'],'--library-path',a.library,*(['--compat=tlc']if profile=='tlc'else[])])
    meta=save(ident,'native',q)
    try:
     if any(flag not in ['-wf','-sf','-nof','-wfNext'] for flag in r['flags']):
      assert r['reference_errors'] and not r['translated'] and b'Unknown command-line option' in gzip.decompress((b/'primary'/r['primary']['stdout']['path']).read_bytes())
      assert q.returncode==2 and not q.stdout.strip() and b'unknown dive translate option' in q.stderr
      outcome='command-rejection-uncredited';detail=f"invalid flag refused: primary CLI exit{r['primary']['exit_code']}/native exit2; no equivalence or artifact credit"
     else:
      doc=json.loads(q.stdout)
      assert q.returncode==doc['exit_code'];assert doc['profile']==profile;assert doc['model_proved']is False and doc['exhaustive']is False;assert doc['result']!='internal-error'
      if not doc['accepted']:
       assert doc['exit_code']in [1,3]and doc['result']in ['unsupported','rejected']and'replacement'not in doc
       if r.get('sany',{}).get('accepted')is False and r['id']!='Echo':
        outcome='invalid-primary-uncredited'
       elif doc['result']=='unsupported':outcome='named-public-refusal'
       elif not r['translated']or r['reference_errors']:outcome='rejection-accounted'
       else:detail=doc
      else:
       assert q.returncode==0 and doc['result']=='replacement'and r['translated']and not r['reference_errors']and r['sany']['accepted']is True
       assert doc['algorithm_checksum_verified']is True and doc['body_checksum_verified']is True
       assert doc['translation_options']==doc['option_context']['effective']and doc['requested_translation_options']==doc['option_context']['requested']
       plan=doc['replacement'];assert plan['algorithm_checksum']['serialized_ast']==(b/'primary'/(r['id']+'.translated.ast')).read_text();assert plan['algorithm_checksum']['checksum']==r['stages']['translated']['crc']
       native=plan['module_source'];file.write_text(native);(a.out/(ident+'.tla')).write_text(native)
       reference=work/'primary'/file.name;reference.parent.mkdir();reference.write_bytes((b/'primary'/(r['id']+'.translated.tla')).read_bytes())
       for tag,path in [('native',file),('primary',reference)]:
        sany=java(['tla2sany.SANY','-error-codes',path]);save(ident,tag+'-sany',sany);assert sany.returncode==0 and b'Semantic errors:'not in sany.stdout
       left=run([a.cst,file]);right=run([a.cst,reference]);save(ident,'native-cst',left);save(ident,'primary-cst',right);l=json.loads(left.stdout);rr=json.loads(right.stdout);assert left.returncode==right.returncode==0 and l['accepted']and rr['accepted']and l['tree']==rr['tree'], 'whole-module CST differs'
       for tag,flags in [('configured',r['flags']),('unconfigured',[])]:
        answers=[]
        for label,path in [('native',file),('primary',reference)]:
         v=java(['pcal.OptionValidatorProbe',*flags,path]);save(ident,label+'-'+tag,v);m=re.search(rb'VALIDATION\t([^\r\n]*)',v.stdout);assert v.returncode==0 and m;answers.append(m[1].decode())
        assert answers[0]==answers[1],answers;statuses[tag]=answers[0]
       assert 'NO_DIVERGENCE'in statuses['configured'];outcome='admitted-exact-cst-sany-validator'
    except (AssertionError,KeyError,ValueError,TypeError)as error:detail=repr(error)
   rows.append({'id':ident,'folder':folder,'source_id':r['id'],'profile':profile,'outcome':outcome,'detail':detail,'native':meta,'validator':statuses})
   if outcome=='disagree':print(ident,detail,flush=True)
   if len(rows)%20==0:print('public',len(rows),flush=True)
summary=dict(collections.Counter(r['outcome']for r in rows));report={'schema_version':1,'selected':len(rows),'native_binary_sha256':sha(a.binary.read_bytes()),'cst_binary_sha256':sha(a.cst.read_bytes()),'script_sha256':sha(Path(__file__).read_bytes()),'oracle_sha256':sha(a.jar.read_bytes()),'family_certified':False,'model_proved':False,'summary':summary,'rows':rows};(a.out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(summary,flush=True);raise SystemExit(bool(summary.get('disagree')or summary.get('retained-grammar-disagreement')))
