#!/usr/bin/env python3
"""Capture single finite process procedure ownership before native implementation."""
import json,re
from pathlib import Path
BASE=Path(__file__).resolve().parent
controls=[]
def case(name,body='F:x:=a; R:return;',params='a=0',locals='',main='L:call First(2); E:skip;',domain='1',binding='=',process_locals='',options=None,extra='',global_vars='variable x=0;'):
    controls.append((name,body,params,locals,main,domain,binding,process_locals,options or [],extra,global_vars))
case('constant_basic')
case('constant_string',domain='"p"')
case('member_one',domain='{1}',binding='\\in')
case('member_two',domain='{1,2}',binding='\\in')
case('member_strings',domain='{"z","a"}',binding='\\in')
case('member_interval',domain='1..2',binding='\\in')
case('member_empty',domain='{}',binding='\\in')
case('member_reverse',domain='2..1',binding='\\in')
case('parameter_default',params='a=3')
case('parameter_missing',params='a')
case('two_parameters',params='a=0,c=a+1',body='F:x:=a+c; R:return;',main='L:call First(2,3); E:skip;')
case('local',locals='variable u=a+1;',body='F:x:=u; R:return;')
case('two_locals',locals='variable u=a+1,v=u+1;',body='F:x:=u+v; R:return;')
case('default_local',locals='variable u;',body='F:x:=a; R:return;')
case('self_local',domain='{1,2}',binding='\\in',locals='variable u=self;',body='F:x:=u; R:return;')
case('self_arg',domain='{1,2}',binding='\\in',main='L:call First(self); E:skip;')
case('constant_self_arg',main='L:call First(self); E:skip;')
case('constant_proc_self',body='F:x:=self; R:return;')
case('member_proc_self',domain='{1,2}',binding='\\in',body='F:x:=self; R:return;')
case('process_local',process_locals='variable y=1;',main='L:call First(y); E:y:=x;')
case('member_process_local',process_locals='variable y=self;',domain='{1,2}',binding='\\in',main='L:call First(y); E:y:=x;')
case('process_local_read',process_locals='variable y=1;',body='F:x:=y+a; R:return;')
case('member_process_local_read',process_locals='variable y=self;',domain='{1,2}',binding='\\in',body='F:x:=y+a; R:return;')
case('process_local_assign',process_locals='variable y=1;',body='F:y:=a; R:return;')
case('member_process_local_assign',process_locals='variable y=self;',domain='{1,2}',binding='\\in',body='F:y:=a; R:return;')
case('repeat_calls',main='L:call First(2); E:call First(3); Z:skip;')
case('self_recursive',body='F:if a>0 then call First(a-1); else x:=a; end if; R:return;')
case('member_recursive',domain='{1,2}',binding='\\in',body='F:if a>0 then call First(a-1); else x:=a; end if; R:return;')
case('self_recursive_local',locals='variable u=a+1;',body='F:if a>0 then call First(a-1); else x:=u; end if; R:x:=x+u; return;')
case('self_tail',body='F:if a>0 then call First(a-1); return; else x:=a; end if; R:return;')
case('member_self_tail',domain='{1,2}',binding='\\in',body='F:if a>0 then call First(a-1); return; else x:=a; end if; R:return;')
case('self_tail_local',locals='variable u=a+1;',body='F:if a>0 then call First(a-1); return; else x:=u; end if; R:return;')
case('tail_parallel_args',params='a=0,c=1',body='F:if a>0 then call First(a-1,a); return; else x:=c; end if; R:return;',main='L:call First(2,3); E:skip;')
case('no_params',params='',main='L:call First(); E:skip;',body='F:x:=1; R:return;')
case('no_params_tail',params='',main='L:call First(); E:skip;',body='F:call First(); return;')
case('missing_return',body='F:x:=a;')
case('main_return',main='L:call First(2); return;')
case('unknown_call',main='L:call Missing(2); E:skip;')
case('wrong_arity',main='L:call First(); E:skip;')
case('member_initializer',locals='variable u \\in {1,2};',body='F:x:=u; R:return;')
case('alias_parameter_local',locals='variable a=1;')
case('alias_process_parameter',process_locals='variable a=1;')
case('global_parameter_alias',params='x=0')
case('process_name_alias',params='Worker=0')
case('undefined_arg',main='L:call First(Missing); E:skip;')
case('undefined_local',locals='variable u=Missing;',body='F:x:=u; R:return;')
case('unbounded_domain',domain='Nat',binding='\\in')
case('weak_option',options=['-wf'])
case('procedure_label_modifier',body='F+:x:=a; R:return;')
case('process_label_modifier',main='L+:call First(2); E:skip;')
case('auto_labels',main='call First(2); skip;',body='x:=a; return;',options=['-label'])
case('stack_depth',body='F:x:=Len(stack); R:return;')
case('indexed_stack_depth',body='F:x:=Len(stack[self]); R:return;')
case('mutual',body='F:if a>0 then call Second(a-1); else x:=a; end if; R:return;',extra='procedure Second(b=0)\nbegin\nS:if b>0 then call First(b-1); else x:=b; end if; T:return;\nend procedure;')
case('unbounded_recursive',body='F:call First(a+1); R:return;')
case('pc_global_alias',global_vars='variable x=0,pc=0;')
def convert(t):
    t=re.sub(r'\bif (.*?) then',lambda m:'if('+m.group(1)+'){',t).replace('else','}else{').replace('end if;','};')
    return t
rows=[];out=BASE/'cases';out.mkdir(exist_ok=True)
for syntax in ('P','C'):
    for name,body,params,locals,main,domain,binding,ploc,options,extra,globals in controls:
        identifier=f'ProcessProcedure_{syntax}_{name}'
        if syntax=='P':
            procs=f'procedure First({params})\n{locals}\nbegin\n{body}\nend procedure;\n{extra}'
            algo=f'--algorithm A\n{globals}\n{procs}\nprocess Worker {binding} {domain}\n{ploc}\nbegin\n{main}\nend process;\nend algorithm;'
        else:
            extra=re.sub(r'procedure (\w+)\((.*?)\)\nbegin\n(.*?)\nend procedure;',lambda m:f'procedure {m[1]}({m[2]}) {{ {convert(m[3])} }};',extra,flags=re.S)
            procs=f'procedure First({params})\n{locals}\n{{ {convert(body)} }};\n{extra}'
            algo=f'--algorithm A {{\n{globals}\n{procs}\nprocess (Worker {binding} {domain})\n{ploc}\n{{ {convert(main)} }}\n}}'
        (out/(identifier+'.tla')).write_text(f'---- MODULE {identifier} ----\nEXTENDS Integers, Sequences, TLC\n(* {algo} *)\n====\n')
        rows.append({'id':identifier,'options':options})
(BASE/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n')
print('Retained',len(rows),'primary source controls')
