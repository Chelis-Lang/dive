#!/usr/bin/env python3
"""Capture-first multiple/nested procedure selection, including invalid/pending boundaries."""
import json
from pathlib import Path
BASE=Path(__file__).resolve().parent
rows=[];out=BASE/'cases';out.mkdir(exist_ok=True)
# name, declaration list (name, parameters, locals, body), main, options
controls=[]
def case(name,declarations,main='L: call First(2); E: skip;',options=None,globals='variable x=0;'):
    controls.append((name,declarations,main,options or [],globals))
def first(body='F: x:=a; R: return;',params='a=0',locals=''):
    return ('First',params,locals,body)
def second(body='S: x:=b; T: return;',params='b=1',locals=''):
    return ('Second',params,locals,body)
case('two_unused',[first(),second()])
case('main_both',[first(),second()],'L:call First(2); E:call Second(3); Z:skip;')
case('order_reverse',[second(),first()])
case('nested',[first('F:call Second(a+1); R:x:=x+a; return;'),second()])
case('nested_local',[first('F:c:=a+1; call Second(c); R:x:=x+c; return;',locals='variable c=1;'),second(locals='variable d=b+1;',body='S:x:=d; T:return;')])
case('nested_return_only',[first('F:call Second(a); R:return;'),second('S:return;')])
case('call_return',[first('F:call Second(a); return;'),second()])
case('call_return_local',[first('F:call Second(c); return;',locals='variable c=a+1;'),second(locals='variable d=b+2;',body='S:x:=d; T:return;')])
case('call_return_assign',[first('F:x:=a; call Second(x+1); return;'),second()])
case('call_return_main',[first(),second()],'L:call First(2); return;')
case('call_goto',[first('F:call Second(a); goto R; R:x:=x+a; return;'),second()])
case('call_done',[first('F:call Second(a); goto Done;'),second()])
case('implicit_caller',[first('F:call Second(a);'),second()])
case('implicit_callee',[first('F:call Second(a); R:return;'),second('S:x:=b;')])
case('repeat_nested',[first('F:call Second(a); R:call Second(a+1); Z:return;'),second()])
case('branch_nested',[first('F:if a>0 then call Second(a); else x:=1; end if; R:return;'),second()])
case('either_nested',[first('F:either call Second(a); or x:=1; end either; R:return;'),second()])
case('with_nested',[first('F:with q=a+1 do call Second(q); end with; R:return;'),second()])
case('while_nested',[first('F:while a<2 do a:=a+1; call Second(a); end while; R:return;'),second()])
case('local_restore',[first('F:c:=a+1; call Second(c); R:x:=c; return;',locals='variable c=9;'),second('S:d:=b+1; x:=d; T:return;',locals='variable d=4;')])
case('foreign_read',[first('F:call Second(a); R:x:=d; return;'),second('S:x:=b; T:return;',locals='variable d=4;')])
case('cross_param_read',[first('F:call Second(a); R:x:=b; return;'),second()])
case('three_chain',[first('F:call Second(a); R:return;'),second('S:call Third(b+1); T:x:=x+b; return;'),('Third','c=0','variable d=c+1;','U:x:=d; V:return;')])
case('four_chain',[first('F:call Second(a); R:return;'),second('S:call Third(b); T:return;'),('Third','c=0','','U:call Fourth(c); V:return;'),('Fourth','d=0','','W:x:=d; Q:return;')])
case('three_tail',[first('F:call Second(a); return;'),second('S:call Third(b); return;'),('Third','c=0','','U:x:=c; V:return;')])
case('no_params',[first('F:call Second(); R:return;',params=''),second('S:x:=1; T:return;',params='')],'L:call First(); E:skip;')
case('default_decls',[first('F:call Second(a); R:return;',params='a',locals='variable c;'),second(locals='variable d;')])
case('two_params',[first('F:call Second(a,c); R:return;',params='a=0,c=1'),second('S:x:=b+d; T:return;',params='b=1,d=2')],'L:call First(2,3); E:skip;')
case('parallel_params',[first('F:a:=c || c:=a; R:call Second(a,c); Z:return;',params='a=0,c=1'),second('S:b:=d || d:=b; T:x:=b+d; return;',params='b=1,d=2')],'L:call First(2,3); E:skip;')
case('unknown_nested',[first('F:call Missing(a); R:return;'),second()])
case('arity_low',[first('F:call Second(); R:return;'),second()])
case('arity_high',[first('F:call Second(a,a); R:return;'),second()])
case('self_recursive',[first('F:call First(a); R:return;'),second()])
case('self_tail',[first('F:call First(a); return;'),second()])
case('mutual',[first('F:call Second(a); R:return;'),second('S:call First(b); T:return;')])
case('unused_cycle',[first(),second('S:call Second(b); T:return;')])
case('conditional_cycle',[first('F:if FALSE then call First(a); else x:=a; end if; R:return;'),second()])
case('duplicate_proc',[first(),('First','b=0','','S:return;')])
case('duplicate_param_scopes',[first(),second(params='a=1',body='S:x:=a; T:return;')])
case('duplicate_locals',[first(locals='variable c=0;'),second(locals='variable c=1;')])
case('global_collision',[first(),second(params='x=1',body='S:return;')])
case('callee_name_collision',[first(),second(params='First=1',body='S:return;')])
case('label_collision',[first(),second('F:x:=b; T:return;')])
case('cross_goto',[first('F:goto S;'),second()])
case('main_proc_goto',[first(),second()],'L:goto F; E:skip;')
case('nested_modifier',[first('F+:call Second(a); R:return;'),second()])
case('fair_algorithm',[first('F:call Second(a); R:return;'),second()],options=['-wf'])
case('strong_algorithm',[first('F:call Second(a); R:return;'),second()],options=['-sf'])
case('auto_labels',[first('call Second(a); return;'),second('x:=b; return;')],main='call First(2); skip;',options=['-label'])
case('unused_invalid',[first(),second('S:x:=Missing; T:return;')])
case('callee_invalid',[first('F:call Second(a); R:return;'),second('S:x:=Missing; T:return;')])
case('reserved_parameter',[first(),second(params='Next=1',body='S:x:=Next; T:return;')])
case('reserved_wrapper',[first(),('Next','b=1','','S:x:=b; T:return;')])
case('read_stack',[first('F:call Second(a); R:x:=Len(stack); return;'),second('S:x:=Len(stack); T:return;')])
case('restore_order',[first('F:c:=a+1; call Second(a); R:x:=c+a; return;',locals='variable c=2;'),second('S:x:=d; T:return;',locals='variable d=b+1;')])
case('parameter_order',[first('F:call Second(a,c); R:return;',params='a=0,c=a+1'),second('S:x:=b+d; T:return;',params='b=1,d=b+1')],'L:call First(2,3); E:skip;')
case('main_empty',[first(),second()],main='')
case('return_transfer_assign',[first('F:a:=a+1; return;'),second()])
case('nested_nonlast',[first('F:call Second(a); x:=1; R:return;'),second()])
case('separate_local_init',[first('F:call Second(a); R:return;'),second('S:x:=d+e; T:return;',locals='variable d=b+1,e=d+1;')])
def convert(source):
    source=source.replace('if a>0 then','if(a>0){').replace('if FALSE then','if(FALSE){').replace('else x:=1; end if;','}else{x:=1;};').replace('else x:=a; end if;','}else{x:=a;};')
    source=source.replace('either call Second(a); or x:=1; end either;','either{call Second(a);}or{x:=1;};')
    source=source.replace('with q=a+1 do','with(q=a+1){').replace('end with;','};')
    return source.replace('while a<2 do','while(a<2){').replace('end while;','};')
for syntax in ['P','C']:
    for name,declarations,main,options,globals in controls:
        id=f'ProcedureGraph_{syntax}_{name}'
        if syntax=='P':
            procs='\n'.join(f'procedure {n}({p})\n{v}\nbegin\n{b}\nend procedure;' for n,p,v,b in declarations)
            algorithm=f'--algorithm A\n{globals}\n{procs}\nbegin\n{main}\nend algorithm;'
        else:
            procs='\n'.join(f'procedure {n}({p})\n{v}\n{{\n{convert(b)}\n}};' for n,p,v,b in declarations)
            algorithm=f'--algorithm A {{\n{globals}\n{procs}\n{{\n{convert(main)}\n}}\n}}'
        source=f'---- MODULE {id} ----\nEXTENDS Integers, Sequences, TLC\n(* {algorithm} *)\n====\n';(out/(id+'.tla')).write_text(source);rows.append({'id':id,'options':options})
(BASE/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n');print(len(rows))
