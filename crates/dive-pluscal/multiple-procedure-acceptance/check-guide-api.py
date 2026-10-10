#!/usr/bin/env python3
"""Compile and run the procedure guide's API example against both saved modules."""
import argparse, gzip, hashlib, json, re, subprocess, tempfile
from pathlib import Path

BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
    guide=(ROOT/'docs/pluscal-multiple-procedures.md').read_text()
    modules=re.findall(r'```tla\n(.*?)\n```',guide,re.S)
    rust=re.findall(r'```rust\n(.*?)\n```',guide,re.S)
    assert len(modules)==2 and len(rust)==1
    deps=args.target/'debug/deps'
    library=max(deps.glob('libdive_api-*.rlib'),key=lambda p:p.stat().st_mtime_ns)
    rows=[];out=BASE/'guide-api';out.mkdir(exist_ok=True)
    for source in modules:
        module=re.search(r'MODULE (\w+)',source).group(1)
        with tempfile.TemporaryDirectory(prefix='dive-procedure-guide-') as tmp:
            work=Path(tmp);(work/(module+'.tla')).write_text(source+'\n')
            program=rust[0].replace('MultipleCallExample.tla',module+'.tla')
            (work/'main.rs').write_text(program)
            subprocess.run(['rustc','--edition=2024',str(work/'main.rs'),'--extern','dive_api='+str(library),'-L','dependency='+str(deps),'-o',str(work/'example')],check=True,capture_output=True)
            p=subprocess.run([str(work/'example')],cwd=work,capture_output=True,timeout=20)
            document=json.loads(p.stdout)
            assert p.returncode==0 and document['result']=='artifact' and document['accepted'] and document['model_proved'] is False and document['exhaustive'] is False
            capture={}
            for stream in ['stdout','stderr']:
                data=getattr(p,stream);name=module+'.'+stream+'.gz';(out/name).write_bytes(gzip.compress(data,mtime=0));capture[stream]={'path':name,'sha256':sha(data)}
            rows.append({'module':module,'source_sha256':sha((source+'\n').encode()),'program_sha256':sha(program.encode()),'result':'artifact','model_proved':False,'exhaustive':False,'exit_code':p.returncode,'capture':capture})
    report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'guide_sha256':sha(guide.encode()),'library_sha256':sha(library.read_bytes()),'controls':2,'rows':rows}
    (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed both executable guide API examples')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--target',type=Path,required=True);main(p.parse_args())
