#!/usr/bin/env python3
"""Complete bounded normalization option selection; both source grammars."""
import json
from pathlib import Path
base=Path(__file__).parent
(base/'cases').mkdir(exist_ok=True)
rows=[]
cases={
 'simple':('x=0','x:=1;','x:=1;'),
 'repeat':('x=0','x:=1; x:=2;','x:=1; x:=2;'),
 'partial':('x=0','x:=1; E: skip;','x:=1; E: skip;'),
 'while':('x=0','while x<2 do x:=x+1; end while; skip;','while (x<2) { x:=x+1; }; skip;'),
 'inner_repeat':('x=0','while x<2 do x:=x+1; x:=x; end while;','while (x<2) { x:=x+1; x:=x; };'),
 'branch':('x=0,y=0','if x=0 then x:=1; else y:=2; end if; x:=2;','if (x=0) { x:=1; } else { y:=2; }; x:=2;'),
 'branch_inner':('x=0','if x=0 then x:=1; x:=2; else skip; end if; skip;','if (x=0) { x:=1; x:=2; } else { skip; }; skip;'),
 'goto':('x=0','goto Done; x:=1;','goto Done; x:=1;'),
 'typed_collision':('x=0','Lbl_1: x:=1; x:=2;','Lbl_1: x:=1; x:=2;'),
 'variable_collision':('Lbl_1=0','Lbl_1:=1;','Lbl_1:=1;'),
 'with':('x=0','with k=1 do x:=k; end with;','with (k=1) { x:=k; };'),
 'with_repeat':('x=0','with k=1 do x:=k; x:=2; end with;','with (k=1) { x:=k; x:=2; };'),
 'default':('x','L: x:=1; E: skip;','L: x:=1; E: skip;'),
 'mixed_default':('x,y=0','L: x:=y; y:=1; E: skip;','L: x:=y; y:=1; E: skip;'),
 'default_loop':('x','L: while TRUE do x:=1; end while;','L: while (TRUE) { x:=1; };'),
 'default_name':('defaultInitValue','L: skip;','L: skip;'),
 'plus':('x=0','L:+ x:=1; E: skip;','L:+ x:=1; E: skip;'),
 'minus':('x=0','L:- x:=1; E: skip;','L:- x:=1; E: skip;'),
 'plus_loop':('x=0','L:+ while TRUE do x:=1; end while;','L:+ while (TRUE) { x:=1; };'),
 'minus_loop':('x=0','L:- while TRUE do x:=1; end while;','L:- while (TRUE) { x:=1; };'),
 'fair_plus_loop':('x=0','L:+ while TRUE do x:=1; end while;','L:+ while (TRUE) { x:=1; };'),
}
for syntax in ('P','C'):
 for name,(decl,p,c) in cases.items():
  variants=[('default',[])]
  if name in {'simple','repeat','partial','typed_collision','while','goto','branch','with_repeat'}:
   variants += [('label',['-label']),('root',['-labelRoot','Step']),('label_root',['-label','-labelRoot','Step']),('report',['-reportLabels'])]
  if name=='simple':variants += [('invalid_root',['-labelRoot','_Step']),('digit_root',['-labelRoot','1Step'])]
  for option,flags in variants:
   ident=f'Norm_{syntax}_{name}_{option}'
   fair='--fair algorithm' if name.startswith('fair_') else '--algorithm'
   body=f'{fair} A variables {decl}; begin {p} end algorithm;' if syntax=='P' else f'{fair} A {{ variables {decl}; {{ {c} }} }}'
   source=f'---- MODULE {ident} ----\nEXTENDS Naturals, TLC\n(* {body} *)\n====\n'
   (base/'cases'/(ident+'.tla')).write_text(source)
   rows.append({'id':ident,'options':flags})
(base/'selection.json').write_text(json.dumps({'schema_version':1,'rows':rows},indent=2)+'\n')
print(len(rows))
