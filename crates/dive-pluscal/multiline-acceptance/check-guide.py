#!/usr/bin/env python3
"""Execute the maintained P/C CLI and Cargo-owned public API guide."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
p=argparse.ArgumentParser();p.add_argument('--native',type=Path,required=True);p.add_argument('--api',type=Path,required=True);a=p.parse_args();guide=(ROOT/'docs/pluscal-multiline.md').read_text();sources=re.findall(r'```tla\n(.*?)\n```',guide,re.S);assert len(sources)==3;out=BASE/'guide';out.mkdir();rows=[]
for syntax,source in zip(['P','C'],sources[:2]):
 with tempfile.TemporaryDirectory(prefix='dive-multiline-guide-') as tmp:
  file=Path(tmp)/'Multiline.tla';original=(source+'\n').encode();file.write_bytes(original)
  for profile in ['dive','tlc']:
   for policy in ['full','tla']:
    for surface in ['cli','cargo-owned-api']:
     cmd=[str(a.native),'translate',str(file),'--rewrite','--checksums',policy,'--output','json'] if surface=='cli' else [str(a.api),str(file)]+(['--body-only'] if policy=='tla' else [])
     if profile=='tlc':cmd.append('--compat=tlc')
     q=subprocess.run(cmd,capture_output=True,timeout=30);doc=json.loads(q.stdout);assert q.returncode==0 and doc['accepted'] is True and doc['result']=='replacement' and doc['model_proved'] is False and doc['exhaustive'] is False and doc['algorithm_checksum_verified']==(policy=='full') and file.read_bytes()==original
     for stream in ['stdout','stderr']:(out/(syntax+'.'+profile+'.'+policy+'.'+surface+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
     rows.append({'syntax':syntax,'profile':profile,'policy':policy,'surface':surface,'outcome':'admitted-nonproof-replacement','stdout_sha256':hashlib.sha256(q.stdout).hexdigest()})
  source=source.replace('Start: x := 1;\n         y := x +\n              2;',sources[2]);file.write_text(source+'\n')
  if syntax=='P':
   for profile in ['dive','tlc']:
    for policy in ['full','tla']:
     q=subprocess.run([str(a.native),'translate',str(file),'--rewrite','--checksums',policy,'--output','json']+(['--compat=tlc'] if profile=='tlc' else []),capture_output=True);doc=json.loads(q.stdout);assert q.returncode==3 and doc['accepted'] is False and doc['model_proved'] is False and 'multiline' in json.dumps(doc);
     for stream in ['stdout','stderr']:(out/('P-junction.'+profile+'.'+policy+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
     rows.append({'syntax':'P-junction','profile':profile,'policy':policy,'outcome':'named-refusal','stdout_sha256':hashlib.sha256(q.stdout).hexdigest()})
report={'schema_version':1,'family_certified':False,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'guide_sha256':hashlib.sha256(guide.encode()).hexdigest(),'native_sha256':hashlib.sha256(a.native.read_bytes()).hexdigest(),'api_example_sha256':hashlib.sha256(a.api.read_bytes()).hexdigest(),'cargo_source':'crates/dive-api/examples/pluscal_multiline.rs','cargo_source_sha256':hashlib.sha256((ROOT/'crates/dive-api/examples/pluscal_multiline.rs').read_bytes()).hexdigest(),'controls':len(rows),'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed',len(rows),'CLI/Cargo-owned API guide controls')
