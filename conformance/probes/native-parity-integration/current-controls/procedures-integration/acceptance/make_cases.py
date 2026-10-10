#!/usr/bin/env python3
"""Complete capture-first C/P procedure selection; retain unsupported and bad artifacts."""
import json
from pathlib import Path

base=Path(__file__).resolve().parent
cases=base/'cases';cases.mkdir(exist_ok=True)
rows=[]
controls=[
('simple','a=0','', 'P: x:=a; R: return;', 'L: call Put(2); E: skip;'),
('no_params','','','P: x:=1; R: return;', 'L: call Put(); E: skip;'),
('default_param','a','','P: x:=a; R: return;', 'L: call Put(2); E: skip;'),
('local','a=0','variable b=1;', 'P: x:=a+b; R: return;', 'L: call Put(2); E: skip;'),
('local_from_param','a=0','variable b=a+1;', 'P: x:=b; R: return;', 'L: call Put(2); E: skip;'),
('default_local','a=0','variable b;', 'P: x:=a; R: return;', 'L: call Put(2); E: skip;'),
('two_params','a=0,b=1','', 'P: x:=a+b; R: return;', 'L: call Put(2,3); E: skip;'),
('repeat','a=0','', 'P: x:=a; R: return;', 'L: call Put(2); E: call Put(3); F: skip;'),
('return_same_label','a=0','', 'P: x:=a; return;', 'L: call Put(2); E: skip;'),
('implicit_return','a=0','', 'P: x:=a;', 'L: call Put(2); E: skip;'),
('return_only','a=0','', 'P: return;', 'L: call Put(2); E: skip;'),
('call_to_done','a=0','', 'P: x:=a; R: return;', 'L: call Put(2);'),
('loop','a=0','', 'P: while a<3 do a:=a+1; end while; R: x:=a; return;', 'L: call Put(0); E: skip;'),
('branch','a=0','', 'P: if a>0 then x:=a; else x:=1; end if; return;', 'L: call Put(2); E: skip;'),
('either','a=0','', 'P: either x:=a; or x:=1; end either; return;', 'L: call Put(2); E: skip;'),
('with','a=0','', 'P: with b=a+1 do x:=b; end with; return;', 'L: call Put(2); E: skip;'),
('local_update','a=0','variable b=1;', 'P: b:=a+1; x:=b; return;', 'L: call Put(2); E: skip;'),
('parallel','a=0,b=1','', 'P: a:=b || b:=a; R: x:=a; return;', 'L: call Put(2,3); E: skip;'),
('initial_expr','a=4','variable b=a+1;', 'P: x:=b; return;', 'L: call Put(2); E: skip;'),
('arity_low','a=0','', 'P: return;', 'L: call Put(); E: skip;'),
('arity_high','a=0','', 'P: return;', 'L: call Put(1,2); E: skip;'),
('unknown','a=0','', 'P: return;', 'L: call Missing(2); E: skip;'),
('return_main','a=0','', 'P: return;', 'L: return; E: skip;'),
('recursive','a=0','', 'P: call Put(a); R: return;', 'L: call Put(2); E: skip;'),
('tail_recursive','a=0','', 'P: call Put(a); return;', 'L: call Put(2); E: skip;'),
('call_goto','a=0','', 'P: x:=a; return;', 'L: call Put(2); goto E; E: skip;'),
('call_follows_assign','a=0','', 'P: x:=a; return;', 'L: x:=1; call Put(x+1); E: skip;'),
('call_missing_follow_label','a=0','', 'P: return;', 'L: call Put(2); x:=4; E: skip;'),
('auto','a=0','', 'x:=a; return;', 'call Put(2); skip;'),
('unused','a=0','', 'P: x:=a; return;', 'L: skip;'),
('unused_invalid','a=0','', 'P: x:=Missing; return;', 'L: skip;'),
('global_param_collision','x=0','', 'P: x:=1; return;', 'L: call Put(2); E: skip;'),
('param_local_collision','a=0','variable a=1;', 'P: x:=a; return;', 'L: call Put(2); E: skip;'),
('duplicate_param','a=0,a=1','', 'P: x:=a; return;', 'L: call Put(2,3); E: skip;'),
('reserved_stack','stack=0','', 'P: return;', 'L: call Put(2); E: skip;'),
('reserved_pc','pc=0','', 'P: return;', 'L: call Put(2); E: skip;'),
('operator_names','Next=0','', 'P: x:=Next; return;', 'L: call Put(2); E: skip;'),
('proc_label_collision','a=0','', 'Put: x:=a; return;', 'L: call Put(2); E: skip;'),
('cross_goto','a=0','', 'P: goto E;', 'L: call Put(2); E: skip;'),
('source_assert','a=0','', 'P: assert a>0; x:=a; return;', 'L: call Put(2); E: skip;'),
('param_read_main','a=0','', 'P: x:=a; return;', 'L: call Put(2); E: x:=a;'),
('local_read_main','a=0','variable b=1;', 'P: x:=a; return;', 'L: call Put(2); E: x:=b;'),
]
def cbody(s):
 s=s.replace('while a<3 do','while(a<3){').replace('end while;','};')
 s=s.replace('if a>0 then','if(a>0){').replace('else x:=1; end if;','}else{x:=1;};')
 s=s.replace('either x:=a; or x:=1; end either;','either{x:=a;}or{x:=1;};')
 s=s.replace('with b=a+1 do','with(b=a+1){').replace('end with;','};')
 return s
for syntax in ['P','C']:
 for name,params,local,body,main in controls:
  id=f'Procedure_{syntax}_{name}'
  if syntax=='P': algorithm=f'--algorithm A\nvariable x=0;\nprocedure Put({params})\n{local}\nbegin\n{body}\nend procedure;\nbegin\n{main}\nend algorithm;'
  else:algorithm=f'--algorithm A {{\nvariable x=0;\nprocedure Put({params})\n{local}\n{{\n{cbody(body)}\n}};\n{{\n{cbody(main)}\n}}\n}}'
  text=f'---- MODULE {id} ----\nEXTENDS Integers, Sequences, TLC\n(* {algorithm} *)\n====\n'
  (cases/f'{id}.tla').write_text(text);rows.append({'id':id,'options':['-label'] if name=='auto' else []})
(base/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n')
print(len(rows))
