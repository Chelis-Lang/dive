#!/usr/bin/env python3
"""Retain recursive procedure admission/normalization controls before native edits."""
import json,re
from pathlib import Path
BASE=Path(__file__).resolve().parent
controls=[]
def case(name,procs,main='L:call First(2); E:skip;',options=None,globals='variable x=0;'):
    controls.append((name,procs,main,options or [],globals))
def first(body,params='a=0',locals=''):
    return ('First',params,locals,body)
def second(body,params='b=0',locals=''):
    return ('Second',params,locals,body)
case('self_base',[first('F:if a>0 then call First(a-1); else x:=x+1; end if; R:x:=x+1; return;')])
case('self_base_zero',[first('F:if a>0 then call First(a-1); else x:=x+1; end if; R:x:=x+1; return;')],'L:call First(0); E:skip;')
case('self_base_negative',[first('F:if a>0 then call First(a-1); else x:=x+1; end if; R:return;')],'L:call First(0-1); E:skip;')
case('self_local',[first('F:if a>0 then call First(a-1); else x:=u; end if; R:x:=x+u; return;',locals='variable u=a+10;')])
case('self_two_locals',[first('F:if a>0 then call First(a-1); else x:=u+v; end if; R:x:=x+u+v; return;',locals='variable u=a+1,v=u+1;')])
case('self_local_mutate',[first('F:u:=a+1; C:if a>0 then call First(a-1); else x:=u; end if; R:x:=x+u; return;',locals='variable u=0;')])
case('self_two_params',[first('F:if a>0 then call First(a-1,c+1); else x:=c; end if; R:return;',params='a=0,c=1')],'L:call First(2,3); E:skip;')
case('self_parameter_order',[first('F:if a>0 then call First(a-1,c+1); else x:=c; end if; R:return;',params='a=0,c=a+1')],'L:call First(2,3); E:skip;')
case('self_parallel_args',[first('F:if a>0 then call First(a-1,a); else x:=c; end if; R:return;',params='a=0,c=1')],'L:call First(2,3); E:skip;')
case('self_twice',[first('F:if a>0 then call First(a-1); else skip; end if; R:if a>0 then call First(a-1); else skip; end if; Z:x:=x+1; return;')])
case('self_with',[first('F:with n=a-1 do if a>0 then call First(n); else x:=x+1; end if; end with; R:return;')])
case('self_either',[first('F:if a>0 then either call First(a-1); or x:=a; end either; else x:=0; end if; R:return;')])
case('self_while',[first('F:while a>0 do call First(a-1); C:a:=a-1; end while; R:x:=x+1; return;')])
case('self_call_goto',[first('F:if a>0 then call First(a-1); goto R; else x:=0; end if; R:return;')])
case('self_goto_done',[first('F:call First(a-1); goto Done;')])
case('self_tail',[first('F:if a>0 then call First(a-1); return; else x:=a; end if; R:return;')])
case('self_tail_local',[first('F:if a>0 then call First(a-1); return; else x:=u; end if; R:return;',locals='variable u=a+10;')])
case('self_tail_two_locals',[first('F:if a>0 then call First(a-1); return; else x:=u+v; end if; R:return;',locals='variable u=a+1,v=u+1;')])
case('self_tail_two_params',[first('F:if a>0 then call First(a-1,c+1); return; else x:=c; end if; R:return;',params='a=0,c=1')],'L:call First(2,3); E:skip;')
case('self_tail_parallel_args',[first('F:if a>0 then call First(a-1,a); return; else x:=c; end if; R:return;',params='a=0,c=1')],'L:call First(2,3); E:skip;')
case('self_tail_assign_global',[first('F:x:=a; if a>0 then call First(a-1); return; else skip; end if; R:return;')])
case('self_tail_changed_local',[first('F:u:=a; call First(a-1); return;',locals='variable u=0;')])
case('self_tail_changed_param',[first('F:a:=a-1; call First(a); return;')])
case('self_tail_no_params',[first('F:call First(); return;',params='')],'L:call First(); E:skip;')
case('self_no_params_unbounded',[first('F:call First(); R:return;',params='')],'L:call First(); E:skip;')
case('self_unbounded',[first('F:call First(a+1); R:return;')])
case('self_tail_counter_unbounded',[first('F:call First(a+1); return;')])
case('mutual_base',[first('F:if a>0 then call Second(a-1); else x:=0; end if; R:x:=x+a; return;'),second('S:if b>0 then call First(b-1); else x:=1; end if; T:x:=x+b; return;')])
case('mutual_reverse',[second('S:if b>0 then call First(b-1); else x:=1; end if; T:return;'),first('F:if a>0 then call Second(a-1); else x:=0; end if; R:return;')])
case('mutual_locals',[first('F:if a>0 then call Second(a-1); else x:=u; end if; R:x:=x+u; return;',locals='variable u=a+1;'),second('S:if b>0 then call First(b-1); else x:=v; end if; T:x:=x+v; return;',locals='variable v=b+2;')])
case('mutual_tail',[first('F:if a>0 then call Second(a-1); return; else x:=a; end if; R:return;'),second('S:if b>0 then call First(b-1); return; else x:=b; end if; T:return;')])
case('mutual_tail_local',[first('F:if a>0 then call Second(a-1); return; else x:=u; end if; R:return;',locals='variable u=a+1;'),second('S:if b>0 then call First(b-1); return; else x:=v; end if; T:return;',locals='variable v=b+2;')])
case('mixed_tail_ordinary',[first('F:if a>0 then call Second(a-1); return; else x:=a; end if; R:return;'),second('S:if b>0 then call First(b-1); else x:=b; end if; T:return;')])
case('mutual_unbounded',[first('F:call Second(a+1); R:return;'),second('S:call First(b+1); T:return;')])
case('mutual_tail_unbounded',[first('F:call Second(a+1); return;'),second('S:call First(b+1); return;')])
case('triple_cycle',[first('F:if a>0 then call Second(a-1); else x:=a; end if; R:return;'),second('S:if b>0 then call Third(b-1); else x:=b; end if; T:return;'),('Third','c=0','','U:if c>0 then call First(c-1); else x:=c; end if; V:return;')],'L:call First(4); E:skip;')
case('triple_tail_cycle',[first('F:if a>0 then call Second(a-1); return; else x:=a; end if; R:return;'),second('S:if b>0 then call Third(b-1); return; else x:=b; end if; T:return;'),('Third','c=0','','U:if c>0 then call First(c-1); return; else x:=c; end if; V:return;')],'L:call First(4); E:skip;')
case('unused_self',[first('F:x:=a; R:return;'),second('S:call Second(b); T:return;')])
case('false_cycle',[first('F:if FALSE then call First(a); else x:=a; end if; R:return;')])
case('default_local',[first('F:if a>0 then call First(a-1); else x:=a; end if; R:return;',locals='variable u;')])
case('repeat_main',[first('F:if a>0 then call First(a-1); else x:=a; end if; R:return;')],'L:call First(2); E:call First(1); Z:skip;')
case('read_stack_depth',[first('F:x:=Len(stack); if a>0 then call First(a-1); else skip; end if; R:return;')])
case('read_stack_tail',[first('F:x:=Len(stack); if a>0 then call First(a-1); return; else skip; end if; R:return;')])
case('fair_self',[first('F:if a>0 then call First(a-1); else x:=a; end if; R:return;')],options=['-wf'])
case('strong_self',[first('F:if a>0 then call First(a-1); else x:=a; end if; R:return;')],options=['-sf'])
case('wrong_arity_self',[first('F:call First(); R:return;')])
case('wrong_arity_tail',[first('F:call First(a,a); return;')])
case('unknown_tail',[first('F:call Missing(a); return;')])
case('missing_return',[first('F:if a>0 then call First(a-1); else x:=a; end if;')])
case('undefined_base',[first('F:if a>0 then call First(a-1); else x:=Missing; end if; R:return;')])
case('duplicate_scope',[first('F:call Second(a); R:return;'),second('S:call First(a); T:return;',params='a=1')])
case('self_modifier',[first('F+:if a>0 then call First(a-1); else x:=a; end if; R:return;')])
case('auto_recursive',[first('if a>0 then call First(a-1); else x:=a; end if; return;')],main='call First(2); skip;',options=['-label'])
case('unused_invalid',[first('F:if a>0 then call First(a-1); else x:=a; end if; R:return;'),second('S:x:=Missing; T:return;')])
case('main_call_return',[first('F:if a>0 then call First(a-1); else x:=a; end if; R:return;')],'L:call First(2); return;')
case('empty_main',[first('F:if a>0 then call First(a-1); else x:=a; end if; R:return;')],main='')
case('reserved_parameter',[first('F:call First(Next); R:return;',params='Next=0')])
case('reserved_wrapper',[('Next','a=0','','F:call Next(a); R:return;')],main='L:call Next(2); E:skip;')
def convert(text):
    text=re.sub(r'\bif (.*?) then',lambda m:'if('+m.group(1)+'){',text)
    text=text.replace('else','}else{').replace('end if;','};')
    text=re.sub(r'\bwhile (.*?) do',lambda m:'while('+m.group(1)+'){',text).replace('end while;','};')
    text=re.sub(r'\bwith (.*?) do',lambda m:'with('+m.group(1)+'){',text).replace('end with;','};')
    return text.replace('either ','either{').replace(' or ','}or{').replace('end either;','};')
rows=[];out=BASE/'cases';out.mkdir(exist_ok=True)
for syntax in ['P','C']:
    for name,procs,main,options,globals in controls:
        id=f'Recursive_{syntax}_{name}'
        if syntax=='P':
            declarations='\n'.join(f'procedure {n}({p})\n{v}\nbegin\n{b}\nend procedure;' for n,p,v,b in procs)
            algorithm=f'--algorithm A\n{globals}\n{declarations}\nbegin\n{main}\nend algorithm;'
        else:
            declarations='\n'.join(f'procedure {n}({p})\n{v}\n{{\n{convert(b)}\n}};' for n,p,v,b in procs)
            algorithm=f'--algorithm A {{\n{globals}\n{declarations}\n{{\n{convert(main)}\n}}\n}}'
        source=f'---- MODULE {id} ----\nEXTENDS Integers, Sequences, TLC\n(* {algorithm} *)\n====\n';(out/(id+'.tla')).write_text(source);rows.append({'id':id,'options':options})
(BASE/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n');print('Retained',len(rows),'source rows')
