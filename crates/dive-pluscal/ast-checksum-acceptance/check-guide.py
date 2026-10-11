#!/usr/bin/env python3
"""Execute documented CLI paths and compile the published Rust example."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
 guide=(ROOT/'docs/pluscal-rewriting.md').read_text();tla=re.findall(r'```tla\n(.*?)\n```',guide,re.S);rust=re.findall(r'```rust\n(.*?)\n```',guide,re.S);assert len(tla)==2 and len(rust)==1
 sources=[tla[0]+'\n',tla[0].split('(*')[0]+tla[1]+'\n\\* Keep this comment.\n====\n'];out=BASE/'guide';out.mkdir(exist_ok=True);rows=[]
 for syntax,source in zip(['P','C'],sources):
  with tempfile.TemporaryDirectory(prefix='dive-rewrite-guide-') as tmp:
   file=Path(tmp)/'Rewrite.tla';file.write_text(source)
   for profile in ['dive','tlc']:
    for policy in ['full','tla']:
     p=subprocess.run([str(args.native),'translate',file.name,'--rewrite','--checksums',policy,'--output','json']+(['--compat=tlc'] if profile=='tlc' else []),cwd=tmp,capture_output=True,timeout=30);doc=json.loads(p.stdout);assert p.returncode==0 and doc['accepted'] and doc['model_proved'] is False and doc['exhaustive'] is False and file.read_text()==source;assert doc['profile']==('tlc+overrides' if profile=='tlc' and policy=='tla' else profile);assert doc['algorithm_checksum_verified']==(policy=='full');captures={}
     for stream in ['stdout','stderr']:
      raw=getattr(p,stream);name=syntax+'.'+profile+'.'+policy+'.'+stream+'.gz';(out/name).write_bytes(gzip.compress(raw,mtime=0));captures[stream]={'path':name,'sha256':sha(raw)}
     rows.append({'syntax':syntax,'profile':profile,'policy':policy,'result':'replacement','source_sha256':sha(source.encode()),'capture':captures})
   for kind in ['write','multiline']:
    altered=source if kind=='write' else source.replace('x = 0','x = 0 +\n1')
    file.write_text(altered);flags=['--rewrite']+(['--write'] if kind=='write' else [])
    p=subprocess.run([str(args.native),'translate',file.name,*flags,'--output','json'],cwd=tmp,capture_output=True,timeout=30);assert p.returncode==3 and file.read_text()==altered,(kind,p.stdout)
    rows.append({'syntax':syntax,'control':kind,'result':'unsupported-no-write','stdout_sha256':sha(p.stdout),'stderr_sha256':sha(p.stderr)})
 deps=args.target/'debug/deps';library=max(deps.glob('libdive_api-*.rlib'),key=lambda p:p.stat().st_mtime_ns)
 program='fn main() {\n'+rust[0]+'\nprintln!("{}", report.to_json());\n}\n'
 with tempfile.TemporaryDirectory(prefix='dive-rewrite-guide-api-') as tmp:
  work=Path(tmp);(work/'main.rs').write_text(program);subprocess.run(['rustc','--edition=2024',str(work/'main.rs'),'--extern','dive_api='+str(library),'-L','dependency='+str(deps),'-o',str(work/'example')],check=True,capture_output=True);p=subprocess.run([str(work/'example')],cwd=tmp,capture_output=True,timeout=20);doc=json.loads(p.stdout);assert p.returncode==0 and doc['accepted'] and doc['model_proved'] is False and doc['body_checksum_verified'] is True
  (out/'rust.stdout.gz').write_bytes(gzip.compress(p.stdout,mtime=0));rows.append({'kind':'compiled-guide-rust','program_sha256':sha(program.encode()),'library_sha256':sha(library.read_bytes()),'stdout_sha256':sha(p.stdout)})
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'guide_sha256':sha(guide.encode()),'native_sha256':sha(args.native.read_bytes()),'controls':len(rows),'family_certified':False,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed eight Full/partial CLI replacements, four no-write/named refusals and compiled Rust guide')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for n in ['native','target']:p.add_argument('--'+n,type=Path,required=True)
 main(p.parse_args())
