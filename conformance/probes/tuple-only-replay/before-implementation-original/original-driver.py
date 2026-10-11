import pathlib,json,hashlib,subprocess,os,time
out=pathlib.Path('/tmp/dive-tuple-only-replay-selection');out.mkdir(exist_ok=True)
binary=pathlib.Path('/tmp/dive-core-snapshot-trace-fresh-runtime/bins/dive')
jar=pathlib.Path('/Users/jeff/code/dive-shipping-cache/sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar')
java=pathlib.Path('/Users/jeff/code/dive-shipping-cache/jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java')
assert hashlib.sha256(jar.read_bytes()).hexdigest()=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
sha=hashlib.sha256(binary.read_bytes()).hexdigest()
cases=[('literal','Tail(<<0,1>>)',''),('definition','Tail(Pair)','Pair == <<0,1>>\n'),('tuple_if','Tail(IF x=0 THEN <<0,1>> ELSE <<2,3>>)',''),('string','Len(Tail("ab"))',''),('mixed_if','Len(Tail(IF x=0 THEN <<0,1>> ELSE "ab"))',''),('concat','<<0>> \\o <<1>>','')]
rows=[]
for name,expr,definitions in cases:
 folder=out/name;folder.mkdir(exist_ok=True);source=folder/'TupleReplay.tla';cfg=folder/'TupleReplay.cfg'
 source.write_text('---- MODULE TupleReplay ----\nEXTENDS Naturals, Sequences\nVARIABLE x\n'+definitions+'Init == x=0\nNext == x\'=IF '+ ('Len('+expr+')' if name not in ['string','mixed_if'] else expr)+' =1 THEN 1 ELSE 2\nInv == x=0\n====\n')
 cfg.write_text('INIT Init\nNEXT Next\nINVARIANT Inv\n')
 env={'PATH':'/usr/bin:/bin','DIVE_CACHE_DIR':'/Users/jeff/code/dive-shipping-cache'}
 def run(label,command):
  start=time.monotonic();r=subprocess.run(command,cwd=folder,env=env,capture_output=True,timeout=60);(folder/(label+'.stdout')).write_bytes(r.stdout);(folder/(label+'.stderr')).write_bytes(r.stderr)
  row={'case':name,'label':label,'command':command,'exit':r.returncode,'seconds':time.monotonic()-start,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()};rows.append(row)
  (out/'rows.json').write_text(json.dumps({'source_commit':'6ff7404e07075e827d9d73e0557ccfbbf238c894','binary_sha256':sha,'rows':rows},indent=2)+'\n')
  return r
 run('primary',[str(java),'-Xmx256m','-cp',str(jar),'tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-metadir',str(folder/'meta'),str(source)])
 for profile in ['tlc','dive']:
  for kind,value in [('actual',1),('wrong',2)]:
   trace=folder/(kind+'.json');trace.write_text(json.dumps({'format':'dive-trace','version':1,'variables':['x'],'states':[{'values':{'x':0}},{'values':{'x':value}}],'ending':{'kind':'invariant','name':'Inv'}}))
   command=[str(binary),'validate-trace',str(source),'--trace',str(trace),'--config',str(cfg),'--seed','0','--fp','0','--output','json']
   if profile=='tlc':command.append('--compat=tlc')
   run(profile+'-'+kind,command)
 assert hashlib.sha256(binary.read_bytes()).hexdigest()==sha
 print(name,'closed',flush=True)
(out/'closure.json').write_text(json.dumps({'all_consumers_closed':True,'rows':len(rows),'binary_sha256':sha,'source_commit':'6ff7404e07075e827d9d73e0557ccfbbf238c894','implementation_changed':False,'family_certified':False},indent=2)+'\n')
