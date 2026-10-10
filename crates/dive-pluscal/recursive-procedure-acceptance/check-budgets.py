#!/usr/bin/env python3
"""Retain clean primary unbounded prefixes and native incomplete bounded capture.

Timed-out primary searches and bounded native searches are never model proofs.
"""
import argparse,gzip,hashlib,json,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
JAR_SHA='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
    assert sha(args.jar.read_bytes())==JAR_SHA
    deps=args.target/'debug/deps';program=BASE/'budgets/probe.rs';binary=BASE/'budgets/probe';command=['rustc','--edition=2024',str(program),'-L','dependency='+str(deps),'-o',str(binary)]
    libraries={}
    for name in ['dive_sema','dive_ir','dive_config','dive_model','dive_check','dive_store','serde_json']:
        lib=max(deps.glob('lib'+name+'-*.rlib'),key=lambda p:p.stat().st_mtime_ns);command+=['--extern',name+'='+str(lib)];libraries[name]=sha(lib.read_bytes())
    subprocess.run(command,check=True,capture_output=True)
    selected=json.loads((BASE/'comparison.json').read_text());rows=[r for r in selected['rows'] if r['outcome']=='token-artifact-match' and r['id'].endswith(('_unbounded','_self_goto_done'))];assert len(rows)==12
    primary=[];controls=[];out=BASE/'budgets'
    for row in rows:
        name=row['id'];source=(BASE/'cases'/(name+'.tla')).read_bytes();doc=json.loads(subprocess.run([str(args.native),'translate','--compat=tlc',name+'.tla','--output','json'],input=None,cwd=BASE/'cases',capture_output=True,check=True).stdout)
        assert doc['result']=='artifact' and doc['model_proved'] is False and doc['exhaustive'] is False
        with tempfile.TemporaryDirectory(prefix='dive-recursive-bounds-') as temporary:
            work=Path(temporary);path=work/(name+'.tla');config=work/(name+'.cfg');config.write_text('INIT Init\nNEXT Next\n'+doc['artifact']['configuration']);path.write_bytes((BASE/'fixtures'/(name+'.translated.tla')).read_bytes())
            command=[str(args.java),'-Xmx128m','-cp',str(args.jar),'tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-config',config.name,path.name]
            try:
                result=subprocess.run(command,cwd=work,capture_output=True,timeout=2);raise ValueError('unbounded primary unexpectedly returned '+str(result.returncode))
            except subprocess.TimeoutExpired as error:
                captures={}
                for key,content in [('stdout',error.stdout or b''),('stderr',error.stderr or b'')]:
                    file=name+'.tlc-prefix.'+key+'.gz';(out/file).write_bytes(gzip.compress(content,mtime=0));captures[key]={'path':file,'sha256':sha(content)}
                primary.append({'id':name,'outcome':'timeout','timeout_seconds':2,'model_proved':False,'exhaustive':False,'captures':captures})
            path.write_text(doc['module_source'])
            for profile in ['tlc','dive']:
                for store in ['fp','exact']:
                    for states,edges,generated in [(0,100,100),(8,100,100),(100,0,100),(100,100,0)]:
                        process=subprocess.run([str(binary),str(path),str(config),profile,store,str(states),str(edges),str(generated)],capture_output=True,timeout=20)
                        if process.returncode:raise ValueError(name+':'+process.stderr.decode())
                        result=json.loads(process.stdout);assert result['result'] in ('resource-limit','unsupported') and result['model_proved'] is False and result['exhaustive'] is False
                        captures={};label=f'{name}.{profile}.{store}.{states}-{edges}-{generated}'
                        for key in ['stdout','stderr']:
                            content=getattr(process,key);file=label+'.'+key+'.gz';(out/file).write_bytes(gzip.compress(content,mtime=0));captures[key]={'path':file,'sha256':sha(content)}
                        controls.append({'id':name,'result':result,'captures':captures})
    report={'schema_version':1,'stage':'incomplete-unbounded-recursive-controls','script_sha256':sha(Path(__file__).read_bytes()),'probe_sha256':sha(program.read_bytes()),'library_sha256':libraries,'native_sha256':sha(args.native.read_bytes()),'oracle_sha256':JAR_SHA,'selection_sha256':sha((BASE/'comparison.json').read_bytes()),'primary_rows':primary,'native_controls':controls,'family_certified':False};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');binary.unlink();print(len(primary),'primary timeouts;',len(controls),'native bounded nonproof controls')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['target','native','java','jar']:p.add_argument('--'+name,type=Path,required=True)
    main(p.parse_args())
