#!/usr/bin/env python3
"""Complete capture-first C/P process/procedure fairness selection."""
import json,re
from pathlib import Path
BASE=Path(__file__).resolve().parent
controls=[]
def proc(name='First',params='a=0',locals='',body='F:x:=x+a; R:return;'):
    return dict(name=name,params=params,locals=locals,body=body)
def process(name,domain,binding='=',locals='',body=None,fair=''):
    return dict(name=name,domain=domain,binding=binding,locals=locals,body=body or ('L:call First(1); E:skip;' if name=='One' else 'M:call First(2); N:skip;'),fair=fair)
def case(name,processes=None,procedures=None,globals='variable x=0;',options=None,fair=False):
    controls.append(dict(name=name,processes=processes or [process('One','1'),process('Two','2')],procedures=procedures or [proc()],globals=globals,options=options or [],fair=fair))

from copy import deepcopy
layouts={
 'constant':[process('One','1')],
 'member':[process('One','{1,2}','\\in')],
 'constants':[process('One','1'),process('Two','2')],
 'mixed':[process('One','1'),process('Two','{2,3}','\\in')],
}
for kind,base in layouts.items():
 case(kind+'_unfair',processes=deepcopy(base))
 for declaration in ['fair ','fair + ']:
  ps=deepcopy(base);ps[0]['fair']=declaration
  case(kind+('_weak' if declaration=='fair ' else '_strong'),processes=ps)
 for option in ['-wf','-sf','-nof','-wfNext']:
  case(kind+'_option_'+option[1:],processes=deepcopy(base),options=[option])
 for option in [None,'-nof','-wf','-sf']:
  case(kind+'_algorithm'+('' if option is None else '_'+option[1:]),processes=deepcopy(base),fair=True,options=[] if option is None else [option])
 for mods in ['F+:x:=x+a; R:return;','F-:x:=x+a; R:return;','F:x:=x+a; R+:return;','F:x:=x+a; R-:return;']:
  key=mods.split(':')[0]+('_rplus' if 'R+' in mods else '_rminus' if 'R-' in mods else '')
  for fair,name in [('', 'unfair'),('fair ','weak'),('fair + ','strong')]:
   ps=deepcopy(base);ps[0]['fair']=fair
   case(kind+'_proc_'+key.replace('+','plus').replace('-','minus')+'_'+name,processes=ps,procedures=[proc(body=mods)])
for mod,name in [('F-:x:=x+a; R-:return;','two_minus'),('F+:x:=x+a; R+:return;','two_plus'),('F-:x:=x+a; R+:return;','mixed')]:
 for kind in ['constant','member','mixed']:
  ps=deepcopy(layouts[kind]);ps[0]['fair']='fair '
  case(kind+'_proc_'+name,processes=ps,procedures=[proc(body=mod)])
for mod,name in [('L+:call First(1); E:skip;','call_plus'),('L-:call First(1); E:skip;','call_minus'),('L+:call First(1); E+:skip;','two_plus'),('L-:call First(1); E-:skip;','two_minus')]:
 for kind in ['constant','member','mixed']:
  ps=deepcopy(layouts[kind]);ps[0]['fair']='fair ';ps[0]['body']=mod
  case(kind+'_process_'+name,processes=ps)
for kind in ['constants','mixed']:
 ps=deepcopy(layouts[kind]);ps[0]['fair']='fair ';ps[1]['fair']='fair + '
 case(kind+'_both_fair',processes=ps)
 case(kind+'_both_fair_option_nof',processes=ps,options=['-nof'])
 case(kind+'_both_fair_option_wf',processes=ps,options=['-wf'])
 case(kind+'_both_fair_option_sf',processes=ps,options=['-sf'])
 case(kind+'_both_fair_algorithm',processes=ps,fair=True)
second=proc('Second','b=0',body='S:x:=x+b; T:return;')
third=proc('Third','c=0',body='U:x:=x+c; V:return;')
for tag,procedures,bodies in [
 ('disjoint',[proc(),second],['L:call First(1); E:skip;','M:call Second(2); N:skip;']),
 ('unused',[proc(),second,third],['L:call First(1); E:skip;','M:call First(2); N:skip;']),
 ('unused_first',[third,proc(),second],['L:call First(1); E:skip;','M:call Second(2); N:skip;']),
 ('two_calls',[proc(),second],['L:call Second(1); E:call First(2); Z:skip;','M:call First(2); N:skip;']),
 ('nested',[proc(body='F:call Second(a); R:return;'),second],['L:call First(1); E:skip;','M:call Second(2); N:skip;']),
 ('nested_modifiers',[proc(body='F-:call Second(a); R+:return;'),proc('Second','b=0',body='S+:x:=x+b; T-:return;')],['L+:call First(1); E:skip;','M-:call Second(2); N:skip;']),
 ('false_branch',[proc(body='F:if FALSE then call Second(a); else x:=x+a; end if; R:return;'),second],['L:call First(1); E:skip;','M:call First(2); N:skip;']),
 ('self_recursion',[proc(body='F:if a>0 then call First(a-1); else x:=x+1; end if; R:return;')],['L:call First(1); E:skip;','M:call First(2); N:skip;']),
 ('self_tail',[proc(body='F+:if a>0 then call First(a-1); return; else x:=x+1; end if; R-:return;')],['L:call First(1); E:skip;','M:call First(2); N:skip;']),
 ('mutual',[proc(body='F:if a>0 then call Second(a-1); else x:=x+1; end if; R:return;'),proc('Second','b=0',body='S:if b>0 then call First(b-1); else x:=x+1; end if; T:return;')],['L:call First(1); E:skip;','M:call Second(2); N:skip;']),
 ('call_goto',[proc(),second],['L:call First(1); goto Z; Z:skip;','M:call Second(2); N:skip;']),
 ('no_calls',[proc(),second],['L:x:=x+1; E:skip;','M:x:=x+2; N:skip;']),
]:
 for kind in ['constants','mixed']:
  ps=deepcopy(layouts[kind]);ps[0]['fair']='fair ';ps[1]['fair']='fair + '
  for p,body in zip(ps,bodies):p['body']=body
  case(kind+'_reach_'+tag,processes=ps,procedures=deepcopy(procedures))
for tag,domain in [('empty','{}'),('reverse','2..1'),('strings','{"z","a"}'),('multi_line','{1,\n2}'),('comment','{1, \* split\n2}')]:
 case('fair_domain_'+tag,processes=[process('One',domain,'\\in',fair='fair ')])
for tag,ps,pr in [
 ('duplicate_label',[process('One','1',fair='fair '),process('Two','2',body='L:call First(2); E:skip;',fair='fair + ')],[proc()]),
 ('cross_local',[process('One','1',locals='variable y=1;',fair='fair '),process('Two','2',body='M:call First(y); N:skip;',fair='fair + ')],[proc()]),
 ('overlap',[process('One','1',fair='fair '),process('Two','1',fair='fair + ')],[proc()]),
 ('unbounded',[process('One','Nat','\\in',fair='fair ')],[proc()]),
 ('invalid_constant_self',[process('One','1',body='L:call First(self); E:skip;',fair='fair ')],[proc()]),
 ('invalid_local_self',[process('One','1',fair='fair ')],[proc(locals='variable u=self;',body='F:x:=x+u; R:return;')]),
 ('invalid_no_param_tail',[process('One','1',body='L:call First(); E:skip;',fair='fair ')],[proc(params='',body='F:call First(); return;')]),
 ('wrong_arity',[process('One','1',body='L:call First(); E:skip;',fair='fair ')],[proc()]),
 ('unknown_call',[process('One','1',body='L:call Missing(1); E:skip;',fair='fair ')],[proc()]),
 ('missing_return',[process('One','1',fair='fair ')],[proc(body='F:x:=x+a;')]),
 ('alias_parameter',[process('One','1',fair='fair ')],[proc(params='x=0')]),
 ('invalid_modifier',[process('One','1',fair='fair ')],[proc(body='F++:x:=x+a; R:return;')]),
]:case(tag,processes=ps,procedures=pr)
assert len({c['name'] for c in controls})==len(controls)
def cbody(text):
    return re.sub(r'\bwhile (.*?) do',lambda m:'while('+m[1]+'){',re.sub(r'\bif (.*?) then',lambda m:'if('+m[1]+'){',text)).replace('else','}else{').replace('end if;','};').replace('end while;','};')
rows=[];out=BASE/'cases';out.mkdir(exist_ok=True)
for syntax in ['C','P']:
 for control in controls:
    name='FairProcedure_'+syntax+'_'+control['name'];parts=['--'+('fair ' if control['fair'] else '')+'algorithm A'+(' {' if syntax=='C' else ''),control['globals']]
    for p in control['procedures']:
        parts.append(f"procedure {p['name']}({p['params']})\n{p['locals']}\n"+('{ '+cbody(p['body'])+' };' if syntax=='C' else 'begin\n'+p['body']+'\nend procedure;'))
    for p in control['processes']:
        header=p['fair']+'process '+(f"({p['name']} {p['binding']} {p['domain']})" if syntax=='C' else f"{p['name']} {p['binding']} {p['domain']}")
        parts.append(header+'\n'+p['locals']+'\n'+('{ '+cbody(p['body'])+' }' if syntax=='C' else 'begin\n'+p['body']+'\nend process;'))
    parts.append('}' if syntax=='C' else 'end algorithm;')
    source=f'---- MODULE {name} ----\nEXTENDS Integers, Sequences, TLC\n(* '+ '\n'.join(parts)+' *)\n====\n';(out/(name+'.tla')).write_text(source);rows.append({'id':name,'options':control['options']})
(BASE/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n')
print('Retained',len(rows),'source controls')
