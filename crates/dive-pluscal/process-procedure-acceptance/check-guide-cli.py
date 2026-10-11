#!/usr/bin/env python3
"""Run both documented modules through public translation in both profiles."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(data):return hashlib.sha256(data).hexdigest()
def main(native):
    guide=(ROOT/'docs/pluscal-process-procedures.md').read_bytes()
    modules=re.findall(r'```tla\n(.*?)\n```',guide.decode(),re.S)
    assert len(modules)==2
    output=BASE/'guide-cli';output.mkdir(exist_ok=True);rows=[]
    for source in modules:
        name=re.search(r'MODULE (\w+)',source).group(1)
        for profile in ['dive','tlc']:
            with tempfile.TemporaryDirectory(prefix='dive-graph-guide-') as tmp:
                work=Path(tmp);(work/(name+'.tla')).write_text(source+'\n')
                p=subprocess.run([str(native),'translate',name+'.tla','--output','json']+(['--compat=tlc'] if profile=='tlc' else []),cwd=work,capture_output=True,timeout=20)
                doc=json.loads(p.stdout)
                assert p.returncode==doc['exit_code']==0 and doc['result']=='artifact' and doc['accepted'] is True and doc['model_proved'] is False and doc['exhaustive'] is False
                capture={}
                for stream in ['stdout','stderr']:
                    data=getattr(p,stream);file=name+'.'+profile+'.'+stream+'.gz';(output/file).write_bytes(gzip.compress(data,mtime=0));capture[stream]={'path':file,'sha256':sha(data)}
                rows.append({'module':name,'profile':profile,'source_sha256':sha((source+'\n').encode()),'result':'artifact','model_proved':False,'exhaustive':False,'exit_code':0,'capture':capture})
    report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'guide_sha256':sha(guide),'native_sha256':sha(native.read_bytes()),'controls':len(rows),'rows':rows}
    (output/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed four guide CLI controls')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--native',type=Path,required=True);main(p.parse_args().native)
