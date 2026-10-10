#!/usr/bin/env python3
"""Retained three-process/storage/control-flow selection; no capture files edited."""
import json
from pathlib import Path
base=Path(__file__).parent/'extra';(base/'cases').mkdir(parents=True,exist_ok=True)
rows=[]
for syn in ('P','C'):
 for pattern,ids in [('ids',['=1','=2','=3']),('sets',['\\in{1,2}','\\in{3,4}','\\in{5}']),('mixed',['\\in{1,2}','=3','\\in{4,5}']),('empty',['\\in{}','=3','\\in{}'])]:
  for control in ('plain','select','parallel','if','either','with','loop','dependent_init','member_init','record_field','fair','modifiers','global_fair','labels','duplicate_name','overlap','crossgoto','escaped_id'):
   ident=f'MultiExtra_{syn}_{pattern}_{control}'
   processes=[]
   for i,(name,binding,local,label) in enumerate(zip('PQR',ids,'xyz','LMN')):
    init=f'variable {local}=0;';body=f'{label}: {local}:={i+1}; {label}E: g:={local};'
    fair='fair' if control in ('fair','modifiers') else ''
    if control=='select':
     init=f'variable {local}=[a |-> 0];';body=f'{label}: {local}.a:={i+1}; {label}E: g:={local}.a;'
    if control=='record_field':body=f'{label}: {local}:=[x |-> 1,y |-> 2,z |-> 3].x; {label}E: g:={local};'
    if control=='parallel':body=f'{label}: {local}:={i+1} || g:=({local}+1)%3; {label}E: skip;'
    if control=='if':body=f'{label}: if {local}=0 then {local}:={i+1}; else {local}:=0; end if; {label}E: g:={local};' if syn=='P' else f'{label}: if ({local}=0) {{ {local}:={i+1}; }} else {{ {local}:=0; }}; {label}E: g:={local};'
    if control=='either':body=f'{label}: either {local}:=1; or {local}:=2; end either; {label}E: g:={local};' if syn=='P' else f'{label}: either {{ {local}:=1; }} or {{ {local}:=2; }}; {label}E: g:={local};'
    if control=='with':body=f'{label}: with b{i} \\in {{1,2}} do {local}:=b{i}; end with; {label}E: g:={local};' if syn=='P' else f'{label}: with (b{i} \\in {{1,2}}) {{ {local}:=b{i}; }}; {label}E: g:={local};'
    if control=='loop':body=f'{label}: while {local}<2 do {local}:={local}+1; end while; {label}E: g:={local};' if syn=='P' else f'{label}: while ({local}<2) {{ {local}:={local}+1; }}; {label}E: g:={local};'
    if control=='dependent_init':init=f'variables {local}=0, {local}d={local}+1;'
    if control=='member_init':init=f'variable {local} \\in {{0,1}};'
    if control=='modifiers':body=body.replace(label+':',label+': +').replace(label+'E:',label+'E: -')
    if control=='labels':body=f'{local}:=1; {label}E: g:={local};'
    if control=='duplicate_name' and i==2:name='Q'
    if control=='overlap' and i==2:binding=ids[0]
    if control=='crossgoto' and i==0:body='L: goto N;'
    if control=='escaped_id' and i==2:binding='="a\\tb"'
    if syn=='P':processes.append(f'{fair} process {name} {binding} {init} begin {body} end process;')
    else:processes.append(f'{fair} process ({name} {binding}) {init} {{ {body} }}')
   marker='--fair algorithm' if control=='global_fair' else '--algorithm'
   alg=f'{marker} A variable g=0; '+ ' '.join(processes)+' end algorithm;' if syn=='P' else f'{marker} A {{ variable g=0; '+ ' '.join(processes)+' }'
   (base/'cases'/(ident+'.tla')).write_text(f'---- MODULE {ident} ----\nEXTENDS Naturals,TLC\n(* {alg} *)\n====\n')
   rows.append({'id':ident,'options':['-label'] if control=='labels' else []})
(base/'selection.json').write_text(json.dumps({'schema_version':1,'rows':rows},indent=2)+'\n')
print(len(rows))
