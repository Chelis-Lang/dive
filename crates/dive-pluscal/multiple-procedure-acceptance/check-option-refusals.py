#!/usr/bin/env python3
"""Public historical-option refusal is explicit and produces no artifact."""
import argparse,gzip,hashlib,json,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
def sha(data):return hashlib.sha256(data).hexdigest()
def main(native):
    manifest=json.loads((BASE/'primary/manifest.json').read_text())
    selected=[r for r in manifest['rows'] if r['options']==['-termination']]
    assert len(selected)==2
    out=BASE/'option-refusals';out.mkdir(exist_ok=True);rows=[]
    for row in selected:
        source=(BASE/'cases'/row['input']).read_bytes();assert sha(source)==row['input_sha256']
        with tempfile.TemporaryDirectory(prefix='dive-pluscal-option-refusal-') as tmp:
            path=Path(tmp)/row['input'];path.write_bytes(source)
            for profile in ['dive','tlc']:
                p=subprocess.run([str(native),'translate',path.name,'--output','json','-termination']+(['--compat=tlc'] if profile=='tlc' else []),cwd=tmp,capture_output=True,timeout=20)
                assert p.returncode==3 and p.stdout==b'' and b'unsupported PlusCal translation option -termination; no artifact produced' in p.stderr
                captures={}
                for stream in ['stdout','stderr']:
                    data=getattr(p,stream);file=row['id']+'.'+profile+'.'+stream+'.gz';(out/file).write_bytes(gzip.compress(data,mtime=0));captures[stream]={'path':file,'sha256':sha(data)}
                rows.append({'id':row['id'],'profile':profile,'exit_code':3,'artifact_produced':False,'model_proved':False,'captures':captures})
    (out/'report.json').write_text(json.dumps({'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'native_sha256':sha(native.read_bytes()),'manifest_sha256':sha((BASE/'primary/manifest.json').read_bytes()),'controls':4,'rows':rows},indent=2)+'\n');print('Passed four public named historical-option refusals')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--native',type=Path,required=True);main(p.parse_args().native)
