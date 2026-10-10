#!/usr/bin/env python3
"""Correct process-local declaration syntax; old malformed C rows remain intact."""
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
def main():
    base=HERE/'process';cases=base/'cases';cases.mkdir(parents=True,exist_ok=True);rows=[]
    for syntax in ('C','P'):
        for kind in ('set','id','mixed','fair','strong','plus','minus','nof','wf','sf','wfnext','auto','with','free_self','foreign','unused_foreign'):
            body='v:=a;'
            if kind=='with':body='with (t=a) { v:=t; }' if syntax=='C' else 'with t=a do v:=t; end with;'
            if kind=='free_self':body='v:=self;'
            if kind=='foreign':body='v:=w;'
            definition='macro Put(v,a)'+(' { '+body+' };' if syntax=='C' else ' begin '+body+' end macro;')
            if kind=='unused_foreign':definition+= 'macro Unused()'+(' { z:=w; };' if syntax=='C' else ' begin z:=w; end macro;')
            def process(name,domain,local,first,last,fairness=''):
                statement=first+': '+('+ ' if kind=='plus' else '- ' if kind=='minus' else '')+'Put('+local+',self); '+last+': skip;'
                if kind=='auto':statement=statement.replace(first+': ','').replace(last+': ','')
                return fairness+'process '+('('+name+domain+') variable '+local+'=0; { '+statement+' }' if syntax=='C' else name+domain+' variable '+local+'=0; begin '+statement+' end process;')
            fairness='fair ' if kind=='fair' else 'fair + ' if kind=='strong' else ''
            domains=('=1','=2') if kind in ('id','free_self') else ('=1','\\in {2,3}') if kind=='mixed' else ('\\in {1,2}','\\in {3,4}')
            processes=process('P',domains[0],'z','L','E',fairness)
            if kind in ('mixed','foreign','unused_foreign'):processes+=process('Q',domains[1],'w','M','F')
            algorithm='--algorithm A '+('{ variable x=0,y=0; '+definition+' '+processes+' }' if syntax=='C' else 'variable x=0,y=0; '+definition+' '+processes+' end algorithm;')
            flags={'nof':['-nof'],'wf':['-wf'],'sf':['-sf'],'wfnext':['-wfNext'],'auto':['-label']}.get(kind,[])
            name='MacroProcess_'+syntax+'_'+kind;source='---- MODULE '+name+' ----\nEXTENDS Integers, TLC\n(* '+algorithm+' *)\n====\n';path=cases/(name+'.tla')
            if path.exists() and path.read_text()!=source:raise ValueError('original source drift')
            path.write_text(source);rows.append({'id':name,'options':flags})
    (base/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n');print('Selected',len(rows),'correct process interactions')
if __name__=='__main__':main()
