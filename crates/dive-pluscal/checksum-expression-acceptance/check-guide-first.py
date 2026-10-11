#!/usr/bin/env python3
"""Execute documented CLI paths and compile the published Rust example."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
 guide=(ROOT/'docs/pluscal-checksum-expressions.md').read_text();tla=re.findall(r'```tla\n(.*?)\n```',guide,re.S);rust=re.findall(r'```rust\n(.*?)\n```',guide,re.S);assert len(tla)==3 and len(rust)==1
 sources=[text+'\n' for text in tla];out=BASE/'guide';out.mkdir();rows=[]
 for syntax,source in zip(['P','C','multiline-refusal'],sources):
  module=re.search(r'MODULE (\w+)',source).group(1)
  with tempfile.TemporaryDirectory(prefix='dive-rewrite-expression-guide-') as tmp:
   file=Path(tmp)/(module+'.tla');file.write_text(source)
   for profile in ['dive','tlc']:
    for policy in ['full','tla']:
     p=subprocess.run([str(args.native),'translate',file.name,'--rewrite','--checksums',policy,'--output','json']+(['--compat=tlc'] if profile=='tlc' else []),cwd=tmp,capture_output=True,timeout=30);doc=json.loads(p.stdout);expected=3 if syntax=='multiline-refusal' else 0;assert p.returncode==expected and doc['accepted']==(expected==0) and doc['model_proved'] is False and doc['exhaustive'] is False and file.read_text()==source
     if expected==0:assert doc['algorithm_checksum_verified']==(policy=='full')
     else:assert 'multiline' in json.dumps(doc) and doc['result']=='unsupported'
     captures={}
     for stream in ['stdout','stderr']:
      raw=getattr(p,stream);name=syntax+'.'+profile+'.'+policy+'.'+stream+'.gz';(out/name).write_bytes(gzip.compress(raw,mtime=0));captures[stream]={'path':name,'sha256':sha(raw)}
     rows.append({'syntax':syntax,'profile':profile,'policy':policy,'result':doc['result'],'source_sha256':sha(source.encode()),'capture':captures})
 deps=args.target/'debug/deps';library=max(deps.glob('libdive_api-*.rlib'),key=lambda p:p.stat().st_mtime_ns)
 program='fn main() {\n'+rust[0]+'\nprintln!("{}", report.to_json());\n}\n'
 with tempfile.TemporaryDirectory(prefix='dive-rewrite-guide-api-') as tmp:
  work=Path(tmp);(work/'main.rs').write_text(program);subprocess.run(['rustc','--edition=2024',str(work/'main.rs'),'--extern','dive_api='+str(library),'-L','dependency='+str(deps),'-o',str(work/'example')],check=True,capture_output=True);p=subprocess.run([str(work/'example')],cwd=tmp,capture_output=True,timeout=20);doc=json.loads(p.stdout);assert p.returncode==0 and doc['accepted'] and doc['model_proved'] is False and doc['body_checksum_verified'] is True
  (out/'rust.stdout.gz').write_bytes(gzip.compress(p.stdout,mtime=0));rows.append({'kind':'compiled-guide-rust','program_sha256':sha(program.encode()),'library_sha256':sha(library.read_bytes()),'stdout_sha256':sha(p.stdout)})
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'guide_sha256':sha(guide.encode()),'native_sha256':sha(args.native.read_bytes()),'controls':len(rows),'family_certified':False,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed eight Full/partial CLI replacements, four multiline-generation refusals and compiled Rust guide')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for n in ['native','target']:p.add_argument('--'+n,type=Path,required=True)
 main(p.parse_args())
