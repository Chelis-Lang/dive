#!/usr/bin/env python3
"""Supplemental no-panic main admission controls; no new artifact certification."""
import argparse, gzip, hashlib, json, subprocess, tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
JAR_SHA='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(b):return hashlib.sha256(b).hexdigest()
def main(args):
    assert sha(args.jar.read_bytes())==JAR_SHA
    out=BASE/'main-admission';out.mkdir(exist_ok=True);rows=[]
    cases={
        'EmptyP':'--algorithm A variable x=0; procedure Put() begin P: return; end procedure; begin end algorithm;',
        'EmptyC':'--algorithm A {variable x=0; procedure Put(){P:return;};{}}',
        'OnlyP':'--algorithm A variable x=0; procedure Put() begin P: return; end procedure; end algorithm;',
        'OnlyC':'--algorithm A {variable x=0; procedure Put(){P:return;};}',
        'CallOnly':'--algorithm A variable x=0; procedure Put() begin P: return; end procedure; begin L:call Put(); end algorithm;',
        'CallUnknown':'--algorithm A variable x=0; procedure Put() begin P:return; end procedure; begin L:call Missing(); end algorithm;',
        'MalformedLabel':'--algorithm A variable x=0; procedure Put() begin P:return; end procedure; begin L:; end algorithm;',
    }
    for name,body in cases.items():
        source=f'---- MODULE {name} ----\nEXTENDS Integers, Sequences, TLC\n(* {body} *)\n====\n';(out/(name+'.tla')).write_text(source)
        with tempfile.TemporaryDirectory(prefix='dive-procedure-main-') as temporary:
            work=Path(temporary);(work/(name+'.tla')).write_text(source)
            calls=[('primary',[str(args.java),'-Xmx256m','-cp',str(args.jar),'pcal.trans',name+'.tla'],None),('core',[str(args.example)],source.encode()),('api',[str(args.native),'translate',name+'.tla','--compat=tlc','--output','json'],None)]
            observations=[]
            for label,command,input_bytes in calls:
                process=subprocess.run(command,cwd=work,input=input_bytes,capture_output=True,timeout=20)
                capture={}
                for stream in ['stdout','stderr']:
                    data=getattr(process,stream);filename=name+'.'+label+'.'+stream+'.gz';(out/filename).write_bytes(gzip.compress(data,mtime=0));capture[stream]={'path':filename,'sha256':sha(data)}
                if label!='primary':
                    document=json.loads(process.stdout)
                    assert document.get('result')!='internal-error' and process.returncode not in [4,101,-6]
                    if label=='api':
                        assert document['accepted'] is (name=='CallOnly')
                        assert document['model_proved'] is False and document['exhaustive'] is False
                    else:
                        assert document.get('accepted', False) is (name=='CallOnly'),(name,document)
                        assert document.get('code') != 'DIVE-P0050'
                        assert document['model_proved'] is False and document['exhaustive'] is False
                    observations.append({'entry':label,'exit_code':process.returncode,'accepted':document.get('accepted'),'code':document.get('diagnostic',{}).get('code'),'capture':capture})
                else:
                    observations.append({'entry':label,'exit_code':process.returncode,'capture':capture})
                    # pcal.trans writes the accepted artifact into the file; restore
                    # the original source before testing the public native API.
                    (work/(name+'.tla')).write_text(source)
            rows.append({'id':name,'source_sha256':sha(source.encode()),'source':name+'.tla','observations':observations,'artifact_certified':False})
    report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'native_sha256':sha(args.native.read_bytes()),'example_sha256':sha(args.example.read_bytes()),'oracle_sha256':JAR_SHA,'controls':len(rows),'rows':rows,'family_certified':False}
    (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed seven no-panic core/API main admission controls')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for key in ['native','example','java','jar']:p.add_argument('--'+key,type=Path,required=True)
    main(p.parse_args())
