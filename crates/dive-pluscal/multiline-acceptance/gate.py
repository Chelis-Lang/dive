#!/usr/bin/env python3
"""Run the complete local workspace gate on this producer source."""
import argparse,json,os,subprocess
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--target',type=Path,required=True);p.add_argument('--log',type=Path,required=True);a=p.parse_args()
env=dict(os.environ,CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',NEXTEST_TEST_THREADS='2',CARGO_TARGET_DIR=str(a.target.resolve()))
commands=[['cargo','fmt','--all','--check'],['python3','-I','scripts/test-validation-import.py'],['python3','-I','scripts/test-frontend-campaign.py'],['python3','-I','scripts/check-parity-ledger.py'],['python3','-I','scripts/test-parity-ledger.py'],['cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings'],['cargo','nextest','run','--workspace','--locked','--test-threads','2'],['cargo','test','--workspace','--doc','--locked'],['cargo','doc','--workspace','--no-deps','--locked'],['cargo','xtask','check-layering'],['cargo','build','-p','dive-trace','--locked'],['cargo','deny','--locked','check']]
with a.log.open('wb') as log:
 for command in commands:
  print('Running',command,flush=True);log.write(('COMMAND '+json.dumps(command)+'\n').encode());log.flush();childenv=env if command[:2]!=['cargo','doc'] else dict(env,RUSTDOCFLAGS='-D warnings');status=subprocess.run(command,cwd=ROOT,env=childenv,stdout=log,stderr=subprocess.STDOUT).returncode;log.write(('STATUS '+str(status)+'\n').encode());log.flush()
  if status:raise SystemExit(status)
print('Complete workspace gate passed',flush=True)
