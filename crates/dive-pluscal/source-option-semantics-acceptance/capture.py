"""Clean primary translation and development-only normalized AST context captures."""
from pathlib import Path
import argparse,base64,gzip,hashlib,json,re,subprocess,tempfile
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--jar',required=True);p.add_argument('--adapter',required=True);a=p.parse_args()
assert hashlib.sha256(Path(a.jar).read_bytes()).hexdigest()==PIN
base=Path(__file__).resolve().parent;out=base/'primary-v2';out.mkdir(exist_ok=False)
def digest(b):return hashlib.sha256(b).hexdigest()
def run(args):return subprocess.run([a.java,'-cp',a.adapter+':'+a.jar,*args],capture_output=True,timeout=30)
def save(stem,q):
 for s in ['stdout','stderr']:(out/(stem+'.'+s+'.gz')).write_bytes(gzip.compress(getattr(q,s),mtime=0))
 return {'exit_code':q.returncode,'stdout_sha256':digest(q.stdout),'stderr_sha256':digest(q.stderr)}
forms=[
 ('weak','\\* PlusCal options (-wf)'),('strong','\\* PlusCal options (-sf)'),('none','\\* PlusCal options (-nof)'),('next','\\* PlusCal options (-wfNext)'),
 ('alias','\\* PlusCal options (wf)'),('alias-next','\\* PlusCal options (wfNext)'),('empty','\\* PlusCal options ()'),('commas','\\* PlusCal options (,, -wf,,)'),
 ('first','\\* PlusCal options (-wf)\n\\* PlusCal options (-sf)'),('first-empty','\\* PlusCal options ()\n\\* PlusCal options (-sf)'),('unrelated-first','\\* PlusCal ignored\n\\* PlusCal options (-wf)'),
 ('line-split','\\* PlusCal\n\\* options (-wf)'),('space-split','\\* PlusCal\n options (-wf)'),('tabs','\\*\tPlusCal\toptions\t(\t-wf\t)'),('adjacent','\\* PlusCal options(-wf)'),
 ('opener','(*PlusCal options (-wf)*)'),('punctuation','(*;PlusCal options (-wf)*)'),('spaced-punctuation','(*; PlusCal options (-wf)*)'),('attached-line','\\*PlusCal options (-wf)'),('unicode-prefix','\\* λ PlusCal options (-wf)'),
 ('labels','\\* PlusCal options (-label, -labelRoot Src)'),('root-only','\\* PlusCal options (-labelRoot Src)'),('root-last','\\* PlusCal options (-labelRoot One, -labelRoot Two, -label)'),
 ('contradictory','\\* PlusCal options (-wf, -sf)'),('repeat','\\* PlusCal options (-wf, -wf)'),('unknown','\\* PlusCal options (-unknown)'),('partial-unknown','\\* PlusCal options (-wf, -unknown)'),
 ('unknown-first','\\* PlusCal options (-unknown, -sf)'),('report','\\* PlusCal options (-reportLabels)'),('missing-open','\\* PlusCal options -wf'),('missing-close','\\* PlusCal options (-wf'),
 ('root-missing','\\* PlusCal options (-labelRoot)'),('root-invalid','\\* PlusCal options (-labelRoot 9Bad)'),('word-suffix','\\* PlusCal optionsX (-wf)'),('token-suffix','\\* PlusCalX options (-wf)'),
 ('after','AFTER'),('translation-block','BLOCK')]
rows=[]
for syntax in ['C','P']:
 for fair in [False,True]:
  for name,comment in forms:
   for opposed in [False,True]:
    ident=f'{syntax}_{"fair" if fair else "plain"}_{name}_{"cli_sf" if opposed else "cli_default"}';module=ident.replace('-','_');flags=['-sf'] if opposed else []
    statement='g:=1;' if name in ['labels','root-only','root-last'] else 'L:g:=1; E:skip;'
    body=f'variables g=0; process P\\in {{1,2}} begin {statement} end process; end algorithm;' if syntax=='P' else f'{{variables g=0; process(P\\in {{1,2}}){{{statement}}}}}'
    marker='--fair algorithm' if fair else '--algorithm'
    alg=f'(* {marker} A {body} *)';before=comment;after=''
    if name=='after':before='';after='\\* PlusCal options (-wf)\n'
    if name=='translation-block':before='\\* BEGIN TRANSLATION\n\\* PlusCal options (-wf)\n\\* END TRANSLATION';after=''
    source=f'---- MODULE {module} ----\nEXTENDS Integers,TLC,Sequences\n{before}\n{alg}\n{after}====\n'
    if name=='tabs':source=source.replace('\n','\r\n')
    sourcebytes=source.encode();(out/(ident+'.tla')).write_bytes(sourcebytes)
    with tempfile.TemporaryDirectory(prefix='dive-source-option-clean-') as tmp:
     file=Path(tmp)/(module+'.tla');file.write_bytes(sourcebytes)
     q=run(['pcal.trans','-nocfg',*flags,str(file)]);artifact=file.read_bytes();(out/(ident+'.translated.tla')).write_bytes(artifact)
     primary=save(ident+'.primary',q)
     file.write_bytes(sourcebytes);stage=run(['pcal.SourceOptionProbe',*flags,str(file)]);stagemeta=save(ident+'.stages',stage)
     parsed={};context=None
     for line in stage.stdout.decode().splitlines():
      fields=line.split('\t')
      if fields[0]=='SOURCE_CONTEXT' and len(fields)==4:context={'status':int(fields[1]),'found':fields[2]=='true','settings':json.loads(base64.b64decode(fields[3]))}
      if fields[0]=='STAGE' and len(fields)==5:
       raw=base64.b64decode(fields[3]);(out/(ident+'.'+fields[1]+'.ast')).write_bytes(raw);(out/(ident+'.'+fields[1]+'.json')).write_bytes(base64.b64decode(fields[4]));parsed[fields[1]]={'crc':fields[2],'sha256':digest(raw)}
     header=re.search(rb'chksum\(pcal\)\s*=\s*"([0-9a-f]+)"\s*/\\\s*chksum\(tla\)\s*=\s*"([0-9a-f]+)"',artifact)
     translated=b'BEGIN TRANSLATION' in artifact
     metadata={'id':ident,'syntax':syntax,'fair_header':fair,'name':name,'flags':flags,'source_sha256':digest(sourcebytes),'output_sha256':digest(artifact),'primary':primary,'stages_process':stagemeta,'context':context,'stages':parsed,'translated':translated,'header':None if header is None else [s.decode() for s in header.groups()],'reference_errors':b'Unrecoverable error' in q.stdout or b'Unrecoverable error' in q.stderr}
     if translated and not metadata['reference_errors'] and header is not None:
      assert 'translated' in parsed and parsed['translated']['crc']==header[1].decode(),metadata
     rows.append(metadata)
    if len(rows)%20==0:print(f'captured {len(rows)}',flush=True)
report={'schema_version':1,'oracle_sha256':PIN,'source_commit':'94d0c5070852b37b801bb8998c436383ab7ee37b','script_sha256':digest(Path(__file__).read_bytes()),'adapter_sha256':digest((Path(a.adapter)/'pcal/SourceOptionProbe.class').read_bytes()),'selected':len(rows),'native_credit':False,'rows':rows};(out/'manifest.json').write_text(json.dumps(report,indent=2)+'\n');print('complete',len(rows),flush=True)
