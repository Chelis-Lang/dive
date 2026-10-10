#!/usr/bin/env python3
"""Default public CLI admission must never credit SANY-invalid raw generation."""
import argparse,gzip,hashlib,json,subprocess,tempfile
from pathlib import Path
HERE=Path(__file__).resolve().parent
def sha(data):return hashlib.sha256(data).hexdigest()
def main(native):
    manifest=json.loads((HERE/'fixtures/manifest.json').read_text());output=HERE/'admission';output.mkdir(exist_ok=True);rows=[]
    selected=[(HERE,row) for row in manifest['rows']]
    manifests={'main':sha((HERE/'fixtures/manifest.json').read_bytes())}
    for directory in ('extra', 'process', 'scope'):
        path=HERE/directory/'fixtures/manifest.json'
        if path.exists():
            selected += [(HERE/directory,row) for row in json.loads(path.read_text())['rows']]
            manifests[directory]=sha(path.read_bytes())
    for base,row in selected:
        if row.get('sany',{}).get('status')!='rejected':continue
        source=(base/'cases'/row['input']).read_bytes()
        if sha(source)!=row['input_sha256']:raise ValueError('invalid-reference source integrity mismatch')
        for profile in ('dive','tlc'):
            with tempfile.TemporaryDirectory(prefix='dive-procedure-admission-') as temporary:
                path=Path(temporary)/row['input'];path.write_bytes(source)
                command=[str(native),'translate',path.name,'--output','json']+(['--compat=tlc'] if profile=='tlc' else [])
                process=subprocess.run(command,cwd=temporary,capture_output=True,timeout=20);doc=json.loads(process.stdout);captures={}
                for stream in ('stdout','stderr'):
                    data=getattr(process,stream);filename=row['id']+'.'+profile+'.'+stream+'.gz';(output/filename).write_bytes(gzip.compress(data,mtime=0));captures[stream]={'path':filename,'sha256':sha(data)}
                passed=process.returncode!=0 and doc.get('exit_code')==process.returncode and doc.get('accepted') is False and doc.get('model_proved') is False and doc.get('exhaustive') is False and doc.get('result') in ('rejected','unsupported') and 'module_source' not in doc and doc.get('result')!='internal-error'
                if not passed:raise ValueError('invalid primary artifact admitted: '+row['id']+':'+profile)
                rows.append({'id':row['id'],'profile':profile,'passed':passed,'result':doc['result'],'stage':doc['stage'],'captures':captures})
    if len(rows)!=8:raise ValueError('all four original invalid source artifacts in both profiles required')
    report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'manifest_sha256':sha((HERE/'fixtures/manifest.json').read_bytes()),'selected_manifest_sha256':manifests,'native_sha256':sha(native.read_bytes()),'family_certified':False,'controls':len(rows),'rows':rows};(output/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Rejected all',len(rows),'public admission controls')
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--native',type=Path,required=True);main(parser.parse_args().native)
