#!/usr/bin/env python3
"""Direct API controls reject every retained primary-invalid module in both profiles."""
import argparse,gzip,hashlib,json,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
PROGRAM='''use dive_api::{TranslationRequest,translate_file};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<_>=std::env::args().collect();
 let request=TranslationRequest{compat_tlc:args[2]=="tlc",..TranslationRequest::default()};
 let report=translate_file(std::path::Path::new(&args[1]),&request)?;
 assert!(!report.accepted()); assert_ne!(report.exit_code(),0);
 println!("{}",report.to_json()); Ok(())
}'''
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
    manifest=json.loads((BASE/'fixtures/manifest.json').read_text());selection=[r for r in manifest['rows'] if r.get('sany',{}).get('status')=='rejected'];assert len(selection)==10
    out=BASE/'api-admission';out.mkdir(exist_ok=True);deps=args.target/'debug/deps';library=max(deps.glob('libdive_api-*.rlib'),key=lambda p:p.stat().st_mtime_ns);controls=[]
    with tempfile.TemporaryDirectory(prefix='dive-recursive-api-') as temporary:
        work=Path(temporary);program=work/'main.rs';program.write_text(PROGRAM);binary=work/'probe'
        subprocess.run(['rustc','--edition=2024',str(program),'--extern','dive_api='+str(library),'-L','dependency='+str(deps),'-o',str(binary)],capture_output=True,check=True)
        for row in selection:
            source=(BASE/'cases'/row['input']).read_bytes();assert sha(source)==row['input_sha256'];path=work/row['input'];path.write_bytes(source)
            for profile in ['dive','tlc']:
                result=subprocess.run([str(binary),str(path),profile],capture_output=True,timeout=20);assert result.returncode==0,result.stderr
                doc=json.loads(result.stdout);assert doc['result'] in ('rejected','unsupported') and doc['accepted'] is False and doc['model_proved'] is False and doc['exhaustive'] is False and doc['exit_code'] in (1,3) and 'module_source' not in doc
                captures={}
                for key in ['stdout','stderr']:
                    content=getattr(result,key);file=row['id']+'.'+profile+'.'+key+'.gz';(out/file).write_bytes(gzip.compress(content,mtime=0));captures[key]={'path':file,'sha256':sha(content)}
                controls.append({'id':row['id'],'profile':profile,'passed':True,'result':doc['result'],'capture':captures})
    report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'program_sha256':sha(PROGRAM.encode()),'manifest_sha256':sha((BASE/'fixtures/manifest.json').read_bytes()),'library_sha256':sha(library.read_bytes()),'controls':len(controls),'rows':controls,'family_certified':False};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed all',len(controls),'direct API invalid-artifact controls')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--target',type=Path,required=True);main(p.parse_args())
