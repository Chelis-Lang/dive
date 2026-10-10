#!/usr/bin/env python3
"""Pinned printable ASCII and permitted string-escape boundaries."""
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
def main():
    base=HERE/'lexical';cases=base/'cases';cases.mkdir(parents=True,exist_ok=True);rows=[]
    contents={'space':' ','tilde':'~','unit_separator':'\x1f','delete':'\x7f','emoji':'🦀','escape_unicode':r'\u03bb','escape_hex':r'\x7f','escaped_tab':r'\t','escaped_newline':r'\n','escaped_formfeed':r'\f','escaped_return':r'\r','escaped_quote':r'\"','escaped_backslash':r'\\'}
    for syntax in ('C','P'):
        for kind,content in contents.items():
            body='Put(x,"'+content+'"); E: skip;'
            algorithm='--algorithm A '+('{ variable x=0; macro Put(v,a) { v:=a; }; { L: '+body+' } }' if syntax=='C' else 'variable x=0; macro Put(v,a) begin v:=a; end macro; begin L: '+body+' end algorithm;')
            name='MacroLexical_'+syntax+'_'+kind;source='---- MODULE '+name+' ----\nEXTENDS Integers, TLC\n(* 🦀 outside quoted expression *)\n(* '+algorithm+' *)\n====\n';path=cases/(name+'.tla')
            if path.exists() and path.read_text()!=source:raise ValueError('immutable string-source drift')
            path.write_text(source);rows.append({'id':name,'options':[]})
    (base/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n');print('Selected',len(rows),'lexical boundaries')
if __name__=='__main__':main()
