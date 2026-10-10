#!/usr/bin/env python3
"""Additional immutable macro source, scope, fairness and lexical controls."""
import importlib.util
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('macro_cases',HERE/'make_cases.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)

def main():
    base=HERE/'extra';cases=base/'cases';cases.mkdir(parents=True,exist_ok=True);rows=[]
    for syntax in ('C','P'):
        for kind in ('fair_modifier','fair_process','auto_if','auto_assert','auto_while','nested_assert','nested_unused','free_self','foreign_local','unused_foreign','cross_goto','with_rename','quoted','escape','latin1','control_string','record_field','free_global','triple','function_capture','quantifier','free_done','multiple_fair','unicode_actual'):
            source,flags=module.case(syntax,'simple')
            definition='macro Put(v, a) { v := a; };' if syntax=='C' else 'macro Put(v, a) begin v := a; end macro;'
            def macro(name,args,body):return 'macro '+name+'('+args+')'+(' { '+body+' };' if syntax=='C' else ' begin '+body+' end macro;')
            if kind in ('fair_modifier','fair_process'):
                source,flags=module.case(syntax,kind);source=source.replace('L+:','L: +').replace('E-:','E: -')
            elif kind=='auto_if':
                source,flags=module.case(syntax,'if');source=source.replace('L: ','').replace('E: ','');flags=['-label']
            elif kind=='auto_assert':
                source,flags=module.case(syntax,'assert');source=source.replace('L: ','').replace('E: ','');flags=['-label']
            elif kind=='auto_while':
                source,flags=module.case(syntax,'while');source=source.replace('L: ','').replace('E: ','');flags=['-label']
            elif kind=='nested_assert':
                source,flags=module.case(syntax,'prior_call');source=source.replace('v := a;', 'assert a>0; v := a;')
            elif kind=='nested_unused':
                source,flags=module.case(syntax,'unused');source=source.replace(definition,definition+macro('Outer','v,a','Put(v,a);'))
            elif kind in ('free_self','foreign_local','unused_foreign','cross_goto','multiple_fair'):
                source,flags=module.case(syntax,'multiple')
                if kind=='free_self':source=source.replace('v := a;', 'v := self;')
                if kind=='foreign_local':source=source.replace('v := a;', 'v := w;')
                if kind=='unused_foreign':source=source.replace(definition,definition+macro('Unused','','z:=w;'))
                if kind=='cross_goto':source=source.replace('v := a;', 'goto F;')
                if kind=='multiple_fair':source=source.replace('process (P','fair + process (P').replace('process (Q','fair process (Q') if syntax=='C' else source.replace('process P','fair + process P').replace('process Q','fair process Q');source=source.replace('L:', 'L: +').replace('M:', 'M: -')
            elif kind=='with_rename':
                source,flags=module.case(syntax,'with');source=source.replace('Put(x, 1)','Put(x, t)')
            elif kind=='quoted':source=source.replace('v := a;', 'v := <<"v", "a", a>>;')
            elif kind=='escape':source=source.replace('v := a;',r'v := <<"\"", "\\", a>>;')
            elif kind=='latin1':source=source.replace('v := a;', 'v := <<"é",a>>;')
            elif kind=='control_string':source=source.replace('v := a;', 'v := <<"\x01",a>>;')
            elif kind=='record_field':source=source.replace('v := a;', 'v := [v |-> a, a |-> a].a;')
            elif kind=='free_global':source=source.replace('v := a;', 'v := y+a;')
            elif kind=='triple':source=source.replace(definition,definition+macro('Outer','v,a','Put(v,a+1);')+macro('Last','v,a','Outer(v,a*2);')).replace('L: Put(x, 1)', 'L: Last(x, 2)')
            elif kind=='function_capture':source=source.replace('v := a;', 'v := [a \\in {1,2} |-> a];')
            elif kind=='quantifier':source=source.replace('v := a;', 'v := \\A a \\in {1,2}: a>0;')
            elif kind=='free_done':source=source.replace(definition,definition+macro('Unused','','goto Done;'))
            elif kind=='unicode_actual':source=source.replace('Put(x, 1)', 'Put(x, ∀ k ∈ {1}: TRUE)')
            name='MacroExtra_'+syntax+'_'+kind
            source='---- MODULE '+name+' ----\nEXTENDS Integers, TLC\n(* '+source+' *)\n====\n'
            path=cases/(name+'.tla')
            if path.exists() and path.read_text()!=source:raise ValueError('retained input drift '+name)
            path.write_text(source);rows.append({'id':name,'options':flags})
    (base/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n');print('Selected',len(rows),'additional controls')
if __name__=='__main__':main()
