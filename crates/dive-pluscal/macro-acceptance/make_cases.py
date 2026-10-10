#!/usr/bin/env python3
"""Complete declared C/P macro substitution, scope and interaction selection."""
import json
from pathlib import Path


def case(syntax, kind):
    def macro(name, params, body):
        header = 'macro ' + name + '(' + ', '.join(params) + ')'
        return header + (' { ' + body + ' };' if syntax == 'C' else ' begin ' + body + ' end macro;')
    declaration = 'variables x=0, y=0;'
    definition = macro('Put', ['v', 'a'], 'v := a;')
    body = 'L: Put(x, 1); E: skip;'
    options = []
    if kind == 'empty': definition = macro('Put', [], ''); body = 'L: Put(); E: skip;'
    elif kind == 'zero': definition = macro('Put', [], 'x:=1;'); body = 'L: Put(); E: skip;'
    elif kind == 'unused': body = 'L: x:=1; E: skip;'
    elif kind == 'compound': body = 'L: Put(x, 1+2); E: skip;'
    elif kind == 'nested_rhs': definition = macro('Put', ['v', 'a'], 'v := 2*a + a;'); body = 'L: Put(x, 1+2); E: skip;'
    elif kind == 'simultaneous': definition = macro('Put', ['a','b'], 'x := <<a,b>>;'); body = 'L: Put(b, 3); E: skip;'
    elif kind == 'duplicate_params': definition = macro('Put', ['a','a'], 'x := a;'); body = 'L: Put(2,3); E: skip;'
    elif kind == 'duplicate_macro': definition += macro('Put', ['v','a'], 'v:=a+1;')
    elif kind == 'prior_call': definition += macro('Outer', ['v','a'], 'Put(v,a+1);'); body = 'L: Outer(x,2); E: skip;'
    elif kind == 'forward_call': definition = macro('Outer',['v','a'],'Put(v,a);') + definition; body='L: Outer(x,1); E: skip;'
    elif kind == 'recursive': definition = macro('Put',['v','a'],'Put(v,a);')
    elif kind == 'unknown': body = 'L: Nope(x,1); E: skip;'
    elif kind == 'arity_low': body = 'L: Put(x); E: skip;'
    elif kind == 'arity_high': body = 'L: Put(x,1,2); E: skip;'
    elif kind == 'literal_lhs': body = 'L: Put(1,2); E: skip;'
    elif kind == 'compound_lhs': body = 'L: Put(x+1,2); E: skip;'
    elif kind == 'index_lhs': declaration='variables x=[i \\in 1..2 |-> 0], y=0;'; body='L: Put(x[1],2); E: skip;'
    elif kind == 'selector_append': declaration='variables x=[i \\in 1..2 |-> [j \\in 1..2 |-> 0]], y=0;'; definition=macro('Put',['v','a'],'v[2]:=a;'); body='L: Put(x[1],2); E: skip;'
    elif kind == 'field_lhs': declaration='variables x=[a |-> 0], y=0;'; body='L: Put(x.a,2); E: skip;'
    elif kind == 'selector_arg': declaration='variables x=[i \\in 1..2 |-> 0], y=0;'; definition=macro('Put',['v','a'],'v[a]:=a+1;'); body='L: Put(x,1); E: skip;'
    elif kind == 'field_rhs': definition=macro('Put',['a'],'x := [a |-> a].a;'); body='L: Put(2); E: skip;'
    elif kind == 'record_type': definition=macro('Put',['a'],'x := [a: {a}];'); body='L: Put(2); E: skip;'
    elif kind == 'string_rhs': definition=macro('Put',['a'],'x := <<"a",a>>;'); body='L: Put(2); E: skip;'
    elif kind == 'bound_capture': definition=macro('Put',['a'],'x := {a \\in 1..2 : a>0};'); body='L: Put(2); E: skip;'
    elif kind == 'with': definition=macro('Put',['v','a'],'with (t = a) { v:=t; }' if syntax=='C' else 'with t=a do v:=t; end with;')
    elif kind == 'with_capture': definition=macro('Put',['v','a'],'with (a = 2) { v:=a; }' if syntax=='C' else 'with a=2 do v:=a; end with;')
    elif kind == 'if': definition=macro('Put',['v','a'],'if (a>0) { v:=a; } else { v:=0; };' if syntax=='C' else 'if a>0 then v:=a; else v:=0; end if;')
    elif kind == 'either': definition=macro('Put',['v','a'],'either { v:=a; } or { v:=0; };' if syntax=='C' else 'either v:=a; or v:=0; end either;')
    elif kind == 'while': definition=macro('Put',['v','a'],'while (v<a) { v:=v+1; };' if syntax=='C' else 'while v<a do v:=v+1; end while;')
    elif kind == 'goto': definition=macro('Put',['v','a'],'goto E;')
    elif kind == 'label_body': definition=macro('Put',['v','a'],'B: v:=a;')
    elif kind == 'assert': definition=macro('Put',['v','a'],'assert a>0; v:=a;')
    elif kind == 'await': definition=macro('Put',['v','a'],'await a>0; v:=a;')
    elif kind == 'print': definition=macro('Put',['v','a'],'print a; v:=a;')
    elif kind == 'parallel': definition=macro('Put',['v','a'],'v:=a || y:=a+1;')
    elif kind == 'repeated': body='L: Put(x,1); E: Put(y,2); F: skip;'
    elif kind == 'same_action_twice': body='L: Put(x,1); Put(x,2); E: skip;'
    elif kind == 'auto_label': body='Put(x,1); skip;';options=['-label']
    elif kind == 'fair_modifier': body='L+: Put(x,1); E-: skip;';options=['-wf']
    elif kind == 'utf8': definition=macro('Put',['v','a'],'v := <<"λ",a>>;')
    elif kind == 'newline_arg': body='L: Put(x,\n1 + 2); E: skip;'
    elif kind == 'tab': definition=definition.replace('v := a;', '\tv := a;')
    elif kind == 'process':
        body=('process (P \\in {1,2}) { variable z=0; L: Put(z,self); E: skip; }' if syntax=='C' else 'process P \\in {1,2} variable z=0; begin L: Put(z,self); E: skip; end process;')
    elif kind == 'fair_process':
        body=('fair process (P \\in {1,2}) { variable z=0; L+: Put(z,self); E-: skip; }' if syntax=='C' else 'fair process P \\in {1,2} variable z=0; begin L+: Put(z,self); E-: skip; end process;')
    elif kind == 'multiple':
        body=('process (P=1) { variable z=0; L: Put(z,1); E: skip; } process (Q \\in {2,3}) { variable w=0; M: Put(w,self); F: skip; }' if syntax=='C' else 'process P=1 variable z=0; begin L: Put(z,1); E: skip; end process; process Q \\in {2,3} variable w=0; begin M: Put(w,self); F: skip; end process;')
    elif kind != 'simple': raise ValueError(kind)
    # The templates above use escaped backslashes independent of Python source.
    declaration=declaration.replace('\\\\','\\');definition=definition.replace('\\\\','\\');body=body.replace('\\\\','\\')
    processes=kind in ['process','fair_process','multiple']
    if syntax=='C': algorithm='--algorithm A { '+declaration+' '+definition+' '+(body if processes else '{ '+body+' }')+' }'
    else: algorithm='--algorithm A '+declaration+' '+definition+' '+(body if processes else 'begin '+body)+' end algorithm;'
    return algorithm, options


def main():
    base=Path(__file__).resolve().parent;cases=base/'cases';cases.mkdir(exist_ok=True);rows=[]
    kinds=['simple','empty','zero','unused','compound','nested_rhs','simultaneous','duplicate_params','duplicate_macro','prior_call','forward_call','recursive','unknown','arity_low','arity_high','literal_lhs','compound_lhs','index_lhs','selector_append','field_lhs','selector_arg','field_rhs','record_type','string_rhs','bound_capture','with','with_capture','if','either','while','goto','label_body','assert','await','print','parallel','repeated','same_action_twice','auto_label','fair_modifier','utf8','newline_arg','tab','process','fair_process','multiple']
    for syntax in ['C','P']:
        for kind in kinds:
            algorithm,options=case(syntax,kind);name='Macro_'+syntax+'_'+kind
            source='---- MODULE '+name+' ----\nEXTENDS Integers, TLC\n(* '+algorithm+' *)\n====\n'
            path=cases/(name+'.tla')
            if path.exists() and path.read_text()!=source:raise ValueError('retained input drift: '+name)
            path.write_text(source);rows.append({'id':name,'options':options})
    (base/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n')
    print('Selected',len(rows),'complete macro source/normalization controls')


if __name__=='__main__':main()
