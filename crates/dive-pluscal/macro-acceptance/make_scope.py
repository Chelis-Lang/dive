#!/usr/bin/env python3
"""Pinned lexical substitution under bindings and qualified field/operator names."""
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
def main():
    base=HERE/'scope';cases=base/'cases';cases.mkdir(parents=True,exist_ok=True);rows=[]
    for syntax in ('P','C'):
        for kind in ('with_shadow','let_shadow','function_shadow','quantifier_shadow','qualified_formal','qualified_actual','record_selector','simultaneous_defined'):
            params='v,a';actual='x,2';body='v:=a;'
            if kind=='with_shadow':body='with (a=3) { v:=a; }' if syntax=='C' else 'with a=3 do v:=a; end with;'
            elif kind=='let_shadow':body='v:=LET a == 3 IN a;'
            elif kind=='function_shadow':body='v:=[a \\in {1,2} |-> a];'
            elif kind=='quantifier_shadow':body='v:=\\A a \\in {1,2}: a>0;'
            elif kind=='qualified_formal':body='v:=I!a;'
            elif kind=='qualified_actual':actual='x,I!a';body='v:=a;'
            elif kind=='record_selector':body='v:= [a |-> a].a;'
            elif kind=='simultaneous_defined':params='a,b';actual='b,3';body='x:=<<a,b>>;'
            definition='macro Put('+params+')'+(' { '+body+' };' if syntax=='C' else ' begin '+body+' end macro;')
            algorithm='--algorithm A '+('{ variables x=0,b=1; '+definition+' { L: Put('+actual+'); E: skip; } }' if syntax=='C' else 'variables x=0,b=1; '+definition+' begin L: Put('+actual+'); E: skip; end algorithm;')
            name='MacroScope_'+syntax+'_'+kind;prefix=''
            if kind.startswith('qualified'):prefix='---- MODULE Helper ----\na == 2\n====\nI == INSTANCE Helper\n'
            source='---- MODULE '+name+' ----\nEXTENDS Integers, TLC\n'+prefix+'(* '+algorithm+' *)\n====\n';path=cases/(name+'.tla')
            if path.exists() and path.read_text()!=source:raise ValueError('retained original scope source drift')
            path.write_text(source);rows.append({'id':name,'options':[]})
    (base/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n');print('Selected',len(rows),'scope controls')
if __name__=='__main__':main()
