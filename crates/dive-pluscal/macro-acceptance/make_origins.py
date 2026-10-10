#!/usr/bin/env python3
"""Composite definition/call locations for inserted macro labels."""
import importlib.util,json
from pathlib import Path
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('macro_cases',HERE/'make_cases.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
def main():
    base=HERE/'origins';cases=base/'cases';cases.mkdir(parents=True,exist_ok=True);rows=[]
    for syntax in ('P','C'):
        for kind in ('with','assert','if','while'):
            source,flags=module.case(syntax,kind)
            source=source.replace('L: Put(x, 1); E: skip;', 'Put(x, 1); skip;')
            if kind!='with':
                outer='macro Outer(v,a)'+(' { Put(v,a); };' if syntax=='C' else ' begin Put(v,a); end macro;')
                if syntax=='C':source=source.replace(' { Put(x, 1);', ' '+outer+' { Outer(x, 1);')
                else:source=source.replace(' begin Put(x, 1);', ' '+outer+' begin Outer(x, 1);')
            name='MacroOrigin_'+syntax+'_'+kind;source='---- MODULE '+name+' ----\nEXTENDS Integers, TLC\n(* '+source+' *)\n====\n';path=cases/(name+'.tla')
            if path.exists() and path.read_text()!=source:raise ValueError('immutable original source drift')
            path.write_text(source);rows.append({'id':name,'options':['-label']})
    (base/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n');print('Selected',len(rows),'macro label source origins')
if __name__=='__main__':main()
