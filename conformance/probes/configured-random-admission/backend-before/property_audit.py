from pathlib import Path
import subprocess,json,hashlib,os,shutil
base=Path('conformance/probes/configured-random-admission/backend-before');cache=Path('/Users/jeff/code/dive-configured-random-cache');sha=lambda b:hashlib.sha256(b).hexdigest()
jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar';assert sha(jar.read_bytes())=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
java=next(cache.glob('jre/*/jdk*/Contents/Home/bin/java'));cli=Path('/Users/jeff/code/dive-view-random-artifacts/dive');env=dict(os.environ);env.pop('DIVE_COMPAT',None);env.pop('DIVE_CHAOS',None)
# The earlier assumption that <>n=3 holds was incorrect: fairness is absent.
old=base/'property-primary-first';old.mkdir(exist_ok=True)
for file in ['property-primary.stdout','property-primary.stderr','FinalConfigured.tla','FinalConfigured.cfg']:
 shutil.copy(base/file,old/file)
(base/'SafeFinal.tla').write_text((base/'FinalConfigured.tla').read_text().replace('MODULE FinalConfigured','MODULE SafeFinal').replace('Live == <> (n=3)','Live == []<> (n>=0)'))
(base/'SafeFinal.cfg').write_text((base/'FinalConfigured.cfg').read_text())
index={'oracle_sha256':sha(jar.read_bytes()),'native_commit':'4ecaa79d33a47e54126feaf171a854a2c77b641b','native_sha256':sha(cli.read_bytes()),'runs':{},'first_primary_assumption':'<>n=3 permits indefinite stutter without fairness; original failed exit0 assertion/raw retained separately, no primary edited.'}
for name,expected in [('FinalConfigured',13),('SafeFinal',0)]:
 args=['-cp',str(jar),'tlc2.TLC','-tool','-seed','7','-fp','0','-workers','1','-lncheck','final','-cleanup','-metadir',f'/tmp/dive-configured-primary-{name}','-config',name+'.cfg',name+'.tla'];p=subprocess.run([str(java),*args],cwd=base,capture_output=True,timeout=30);key=name+'-primary';(base/(key+'.stdout')).write_bytes(p.stdout);(base/(key+'.stderr')).write_bytes(p.stderr);index['runs'][key]={'exit':p.returncode,'args':['java@pinned-jre','-cp','jar@'+sha(jar.read_bytes()),*args[2:]]};assert p.returncode==expected,(name,p.returncode);print(key,p.returncode)
for lasso in ['short','tlc']:
 for compat in [False,True]:
  args=['check','SafeFinal.tla','--lncheck','final','--lasso',lasso,'--seed','7','--fp','0','--workers','1','--output','json'];args+=['--compat=tlc'] if compat else []
  p=subprocess.run([str(cli),*args],cwd=base,env=env,capture_output=True,timeout=30);key='SafeFinal-'+lasso+'-'+('tlc' if compat else 'dive');(base/(key+'.stdout')).write_bytes(p.stdout);(base/(key+'.stderr')).write_bytes(p.stderr);index['runs'][key]={'args':args,'exit':p.returncode};j=json.loads(p.stdout);print(key,p.returncode,j['verdict'])
index['files']={f.name:sha(f.read_bytes()) for f in base.iterdir() if f.is_file() and f.name.startswith(('SafeFinal','FinalConfigured'))}
(base/'property-audit-index.json').write_text(json.dumps(index,indent=2)+'\n')
shutil.copy('/tmp/dive-configured-property-audit.py',base/'property_audit.py')
