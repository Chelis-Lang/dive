import hashlib,json,os,subprocess,time,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve()
base=Path(__file__).resolve().parent
release=Path(sys.argv[2]).resolve()
cache=Path(os.environ['DIVE_CACHE_DIR']).resolve()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
before={'schema_version':1,'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'source_matrix':json.loads((root/'conformance/probes/tlc-final-check/source-matrix.json').read_text()),'selection':{'corpus':'examples','tier':'small','filters':[],'workers':[1],'timeout_seconds':600,'jobs':1},'implementation':'dive-final-tlc','binary_sha256':{n:sha(release/n) for n in ['dive','dive-conformance']},'launcher_sha256':sha(base/'capture-cli.py'),'scope':'Fresh unchanged full small selection on the integrated final-only source. The actual executable and transparent archival launcher are separately hashed. Refusals, failures and unavailable facts earn no parity credit.'}
(base/'before.json').write_text(json.dumps(before,indent=2)+'\n')
env=os.environ|{'DIVE_CACHE_DIR':str(cache),'DIVE_CAPTURE_BINARY':str(release/'dive')}
args=[str(release/'dive-conformance'),'run','--impl','dive-final-tlc','--impl-profile','tlc','--corpus','examples','--tier','small','--workers','1','--jobs','1','--timeout','600','--no-sany','--no-summary','--dive-bin',str(base/'capture-cli.py'),'--report-dir',str(base/'reports')]
started=time.monotonic()
with (base/'run.log').open('wb') as out:
 code=subprocess.run(args,cwd=root,env=env,stdout=out,stderr=subprocess.STDOUT).returncode
for name,digest in before['binary_sha256'].items():assert sha(release/name)==digest,name
for name,digest in before['source_matrix']['source_sha256'].items():assert sha(root/name)==digest,name
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==before['source_commit']
artifacts={str(p.relative_to(base)):sha(p) for p in sorted(base.rglob('*')) if p.is_file() and p.name!='after.json'}
after={'source_and_binaries_unchanged':True,'harness_exit':code,'elapsed_seconds':time.monotonic()-started,'artifact_sha256':artifacts}
(base/'after.json').write_text(json.dumps(after,indent=2)+'\n')
print(f'Fresh complete study exited {code}; exact source/binaries verified unchanged')
