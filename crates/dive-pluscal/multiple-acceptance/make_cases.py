#!/usr/bin/env python3
"""Reproduce the initial complete two-process selection; never edit captures."""
from pathlib import Path
import json
base=Path(__file__).parent;(base/'cases').mkdir(exist_ok=True)
rows=[]
def add(syn,name,pid,qid,pbody='L: x:=1; E: g:=x;',qbody='M: y:=2; F: g:=y;',pvars='variable x=0;',qvars='variable y=0;',fairp='',fairq='',flags=(),gfair=False):
 ident='Multi_'+syn+'_'+name;marker='--fair algorithm' if gfair else '--algorithm'
 if syn=='P':alg=f'{marker} A variable g=0; {fairp} process P {pid} {pvars} begin {pbody} end process; {fairq} process Q {qid} {qvars} begin {qbody} end process; end algorithm;'
 else:alg=f'{marker} A {{ variable g=0; {fairp} process (P {pid}) {pvars} {{ {pbody} }} {fairq} process (Q {qid}) {qvars} {{ {qbody} }} }}'
 content=f'---- MODULE {ident} ----\nEXTENDS Naturals,TLC\n(* {alg} *)\n====\n';path=base/'cases'/(ident+'.tla')
 if path.exists() and path.read_text()!=content:raise ValueError('source changed '+ident)
 path.write_text(content);rows.append({'id':ident,'options':list(flags)})
for syn in ['P','C']:
 for name,p,q in [('ids','=1','=2'),('strings','="p"','="q"'),('mixed','=1','="q"'),('idset','=1','\\in{2,3}'),('setid','\\in{1,2}','=3'),('sets','\\in{1,2}','\\in{3,4}'),('intervals','\\in1..2','\\in3..4'),('empty','\\in{}','\\in{}'),('empty_id','\\in{}','=1'),('dup_set','\\in{1,1}','\\in{2,2}'),('overlap','=1','\\in{1,2}'),('overlap_sets','\\in{1,2}','\\in{2,3}'),('nonliteral','=1','\\in1..g')]:add(syn,name,p,q)
 for name,p,q in [('ids','=1','=2'),('idset','=1','\\in{2,3}'),('sets','\\in{1,2}','\\in{3,4}')]:
  for kind,fp,fq in [('weak','fair','fair'),('strong','fair +','fair +'),('mixed','fair','fair +')]:add(syn,name+'_'+kind,p,q,fairp=fp,fairq=fq)
  for flag in ['-nof','-wf','-sf','-wfNext']:add(syn,name+'_flag_'+flag[1:],p,q,flags=[flag])
  add(syn,name+'_globalfair',p,q,gfair=True)
  add(syn,name+'_modifiers',p,q,pbody='L: + x:=1; E: - g:=x;',qbody='M: - y:=2; F: + g:=y;',fairp='fair',fairq='fair')
  add(syn,name+'_default',p,q,pvars='variable x;',qvars='variable y;')
  add(syn,name+'_parallel',p,q,pbody='L: x:=1 || g:=2; E: skip;',qbody='M: y:=2 || g:=1; F: skip;')
 add(syn,'crossgoto','=1','=2',pbody='L: goto M;')
 add(syn,'crosslocal','=1','=2',pbody='L: x:=y;')
 add(syn,'labelcollision','=1','=2',qbody='L: y:=2; F: g:=y;')
 add(syn,'localcollision','=1','=2',qvars='variable x=0;',qbody='M: x:=2; F: g:=x;')
 add(syn,'processcollision','=1','=2',pvars='variable Q=0;',pbody='L: Q:=1;')
 inf=('L: while TRUE do x:=(x+1)%2; end while;' if syn=='P' else 'L: while (TRUE) { x:=(x+1)%2; };')
 add(syn,'infinite','=1','=2',pbody=inf,qbody=inf.replace('L:','M:').replace('x','y'))
 add(syn,'infinite_one','=1','=2',pbody=inf)
 add(syn,'self','=1','\\in{2,3}',qbody='M: y:=self; F: g:=y;')
(base/'selection.json').write_text(json.dumps({'schema_version':1,'rows':rows},indent=2)+'\n');print(len(rows))
