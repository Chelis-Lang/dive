#!/usr/bin/env python3
"""Execute P/C process-option CLI and Cargo-owned API examples."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
p=argparse.ArgumentParser();p.add_argument('--native',type=Path,required=True);p.add_argument('--api',type=Path,required=True);a=p.parse_args();guide=(ROOT/'docs/pluscal-process-checksum-options.md').read_text();sources=re.findall(r'```tla\n(.*?)\n```',guide,re.S);assert len(sources)==2;out=BASE/'guide';out.mkdir();rows=[]
for syntax,source in zip(['P','C'],sources):
 with tempfile.TemporaryDirectory(prefix='dive-process-options-guide-') as tmp:
  file=Path(tmp)/'ProcessOptions.tla';original=(source+'\n').encode();file.write_bytes(original)
  for profile in ['dive','tlc']:
   for fairness in ['-wf','-nof']:
    for surface in ['cli','cargo-owned-api']:
     flags=['-label','-labelRoot','Auto',fairness]
     cmd=[str(a.native),'translate',str(file),'--rewrite','--checksums','full','--output','json',*flags] if surface=='cli' else [str(a.api),str(file),*flags]
     if profile=='tlc':cmd.append('--compat=tlc')
     q=subprocess.run(cmd,capture_output=True,timeout=30);doc=json.loads(q.stdout);assert q.returncode==0 and doc['accepted'] is True and doc['model_proved'] is False and doc['exhaustive'] is False and doc['algorithm_checksum_verified'] is True and doc['translation_options']['insert_missing_labels'] is True and doc['translation_options']['label_root']=='Auto' and doc['translation_options']['fairness']==('Weak' if fairness=='-wf' else 'None') and file.read_bytes()==original
     assert 'Auto1' in doc['replacement']['module_source'] and 'Auto2' in doc['replacement']['module_source']
     for stream in ['stdout','stderr']:(out/(syntax+'.'+profile+'.'+fairness+'.'+surface+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
     rows.append({'syntax':syntax,'profile':profile,'fairness':fairness,'surface':surface,'outcome':'admitted-nonproof','stdout_sha256':hashlib.sha256(q.stdout).hexdigest()})
report={'schema_version':1,'family_certified':False,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'guide_sha256':hashlib.sha256(guide.encode()).hexdigest(),'native_sha256':hashlib.sha256(a.native.read_bytes()).hexdigest(),'api_sha256':hashlib.sha256(a.api.read_bytes()).hexdigest(),'cargo_source':'crates/dive-api/examples/pluscal_process_options.rs','cargo_source_sha256':hashlib.sha256((ROOT/'crates/dive-api/examples/pluscal_process_options.rs').read_bytes()).hexdigest(),'controls':len(rows),'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed',len(rows),'guide controls')
