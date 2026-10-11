"""Materialize the complete captured selection into a fresh external directory."""
import argparse,importlib.util,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);p.add_argument('--epoch',choices=['corrected','first','nof'],default='corrected');a=p.parse_args()
s=importlib.util.spec_from_file_location('reference_bytes',Path(__file__).with_name('nof_context.py'if a.epoch=='nof'else'capture_bytes.py'));m=importlib.util.module_from_spec(s);s.loader.exec_module(m);records=m.load();a.out.mkdir(exist_ok=False)
for prefix in (['cases','primary']if a.epoch=='nof'else['cases','primary','exploded']):
 original='first-'+prefix if a.epoch=='first'else prefix
 for name,data in records.items():
  if name.startswith(original+'/'):
   dest=a.out/prefix/name[len(original)+1:];dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
print('Materialized immutable reference settings without native credit:',56 if a.epoch=='nof'else 242)
