import subprocess,os,sys
label=sys.argv[1]
commands=[['cargo','fmt','--all','--check'],['cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings'],['cargo','nextest','run','--workspace','--locked','--profile','ci'],['cargo','test','--workspace','--doc','--locked'],['cargo','doc','--workspace','--no-deps','--locked'],['cargo','xtask','check-layering'],['cargo','build','-p','dive-trace','--locked'],['cargo','deny','--locked','check']]
for i,cmd in enumerate(commands):
 print('RUN',' '.join(cmd),flush=True);env=os.environ.copy()
 if cmd[1]=='doc':env['RUSTDOCFLAGS']='-D warnings'
 path=f'/tmp/dive-{label}-gate-{i}.log'
 with open(path,'w') as f:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,env=env)
 print('RESULT',r.returncode,path,flush=True)
 if r.returncode:
  print(open(path).read()[-10000:]);raise SystemExit(r.returncode)
