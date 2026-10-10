#!/usr/bin/env python3
"""Complete curated fairness selection captured before native changes."""
import json
from pathlib import Path
base = Path(__file__).parent
(base/'cases').mkdir(exist_ok=True)
rows=[]
def case(name, syntax, binding, fair, labels, flags=(), algorithm=False, body=None):
    ident='Fair_'+syntax+'_'+name
    first,second=labels
    p=body or f'L{first}: x:=1; E{second}: g:=x;'
    globals_='variables g=0;'
    locals_='variables x=0;'
    marker='--fair algorithm' if algorithm else '--algorithm'
    if syntax=='P':
        src=f'{marker} A {globals_} {fair} process P {binding} {locals_} begin {p} end process; end algorithm;'
    else:
        src=f'{marker} A {{ {globals_} {fair} process (P {binding}) {locals_} {{ {p} }} }}'
    (base/'cases'/(ident+'.tla')).write_text(f'---- MODULE {ident} ----\nEXTENDS Naturals,TLC\n(* {src} *)\n====\n')
    rows.append({'id':ident,'options':list(flags)})
for syntax in ('P','C'):
    for domain,binding in [('id','=1'),('set','\\in{1,2}')]:
        for kind,fair in [('unfair',''),('weak','fair'),('strong','fair +')]:
            for mod,labels in [('plain',('','')),('plus',('+','')),('minus',('-','')),('both',('+','-')),('minus_pair',('-','-'))]:
                case(f'{domain}_{kind}_{mod}',syntax,binding,fair,labels)
            case(f'{domain}_{kind}_fair_algorithm',syntax,binding,fair,('+','-'),algorithm=True)
            for option in ('nof','wf','sf','wfNext'):
                case(f'{domain}_{kind}_option_{option}',syntax,binding,fair,('+','-'),['-'+option])
    for kind,fair in [('unfair',''),('weak','fair'),('strong','fair +')]:
        for mod,label in [('plain',''),('plus','+'),('minus','-')]:
            body=(f'L{label}: while TRUE do x:=(x+1)%2; end while;' if syntax=='P' else f'L{label}: while (TRUE) {{ x:=(x+1)%2; }};')
            case(f'infinite_{kind}_{mod}',syntax,'\\in{1,2}',fair,('',''),body=body)
    case('missing_default',syntax,'\\in{1,2}','fair',('',''),body='x:=1; E+: g:=x;')
    case('missing_label',syntax,'\\in{1,2}','fair',('',''),['-label'],body='x:=1; E+: g:=x;')
    case('empty_domain',syntax,'\\in{}','fair',('+','-'))
    case('string_id',syntax,'="worker"','fair',('+','-'))
    case('label_action_keyword',syntax,'\\in{1,2}','fair',('',''),body='WF+: x:=1; SF-: g:=x;')
    # Keep two-block and procedure evidence visible without admission credit.
    ident='Fair_'+syntax+'_two_blocks'
    if syntax=='P': alg='--algorithm A variable g=0; fair process P=1 begin L: g:=1; end process; fair process Q=2 begin M: g:=2; end process; end algorithm;'
    else: alg='--algorithm A { variable g=0; fair process (P=1) { L: g:=1; } fair process (Q=2) { M: g:=2; } }'
    (base/'cases'/(ident+'.tla')).write_text(f'---- MODULE {ident} ----\nEXTENDS Naturals,TLC\n(* {alg} *)\n====\n')
    rows.append({'id':ident,'options':[]})
(base/'selection.json').write_text(json.dumps({'schema_version':1,'rows':rows},indent=2)+'\n')
print(len(rows))
# Preserve the originally measured modifier-before-colon forms as rejection
# controls; positive source puts the modifier after the label's colon.
positive=base/'positive'
(positive/'cases').mkdir(parents=True,exist_ok=True)
corrected=[]
for row in rows:
    ident=row['id']; name=ident.replace('Fair_','FairPositive_',1)
    source=(base/'cases'/(ident+'.tla')).read_text().replace(ident,name)
    for label in ('L','E','WF','SF'):
        for modifier in ('+','-'):
            source=source.replace(label+modifier+':',label+': '+modifier)
    (positive/'cases'/(name+'.tla')).write_text(source)
    corrected.append({'id':name,'options':row['options']})
(positive/'selection.json').write_text(json.dumps({'schema_version':1,'rows':corrected},indent=2)+'\n')
