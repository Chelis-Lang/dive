#!/usr/bin/env python3
"""Retained process translation selection, before native implementation."""
import json
from pathlib import Path

base = Path(__file__).parent
(base / 'cases').mkdir(exist_ok=True)
rows = []
# Global declarations, local declarations, P body, C body.
cases = {
    'simple': ('g=0', '', 'L: g:=1; E: skip;', 'L: g:=1; E: skip;'),
    'local': ('g=0', 'x=0', 'L: x:=x+1; E: g:=x;', 'L: x:=x+1; E: g:=x;'),
    'self': ('g=0', 'x=self', 'L: g:=x; E: x:=self+1;', 'L: g:=x; E: x:=self+1;'),
    'local_member': ('g=0', 'x\\in{0,1}', 'L: g:=x; E: x:=0;', 'L: g:=x; E: x:=0;'),
    'local_dependency': ('g=0', 'x=0,y=x+1', 'L: g:=y; E: x:=g;', 'L: g:=y; E: x:=g;'),
    'parallel': ('g=0', 'x=0,y=0', 'L: x:=y+1 || y:=x+1; E: g:=x+y;', 'L: x:=y+1 || y:=x+1; E: g:=x+y;'),
    'sequential': ('g=0', 'x=0', 'L: x:=1; g:=x; E: skip;', 'L: x:=1; g:=x; E: skip;'),
    'select': ('g=0', 'x=<<0,0>>', 'L: x[1]:=self; E: g:=x[1];', 'L: x[1]:=self; E: g:=x[1];'),
    'record': ('g=0', 'x=[a|->0]', 'L: x.a:=self; E: g:=x.a;', 'L: x.a:=self; E: g:=x.a;'),
    'if': ('g=0', 'x=0', 'L: if x=0 then x:=1; else g:=2; end if; E: skip;', 'L: if (x=0) { x:=1; } else { g:=2; }; E: skip;'),
    'either': ('g=0', 'x=0', 'L: either x:=1; or g:=2; end either; E: skip;', 'L: either { x:=1; } or { g:=2; }; E: skip;'),
    'with': ('g=0', 'x=0', 'L: with k\\in{0,1} do x:=k; end with; E: g:=x;', 'L: with (k\\in{0,1}) { x:=k; }; E: g:=x;'),
    'loop': ('g=0', 'x=0', 'L: while x<2 do x:=x+1; end while; E: g:=x;', 'L: while (x<2) { x:=x+1; }; E: g:=x;'),
    'infinite': ('g=0', 'x=0', 'L: while TRUE do x:=(x+1)%2; end while;', 'L: while (TRUE) { x:=(x+1)%2; };'),
    'goto': ('g=0', 'x=0', 'L: x:=1; goto E; E: g:=x;', 'L: x:=1; goto E; E: g:=x;'),
    'await': ('g=0', 'x=0', 'L: await x=0; x:=1; E: skip;', 'L: await (x=0); x:=1; E: skip;'),
    'assert': ('g=0', 'x=0', 'L: x:=1; assert x=0; E: skip;', 'L: x:=1; assert x=0; E: skip;'),
    'default_global': ('g', 'x=0', 'L: g:=x; E: skip;', 'L: g:=x; E: skip;'),
    'default_local': ('g=0', 'x', 'L: x:=1; E: g:=x;', 'L: x:=1; E: g:=x;'),
    'no_globals': ('', 'x=0', 'L: x:=1; E: skip;', 'L: x:=1; E: skip;'),
    'no_variables': ('', '', 'L: skip; E: skip;', 'L: skip; E: skip;'),
    'implicit': ('g=0', 'x=0', 'x:=1; g:=x;', 'x:=1; g:=x;'),
    'partial_labels': ('g=0', 'x=0', 'x:=1; E: g:=x;', 'x:=1; E: g:=x;'),
    'shadow': ('x=0', 'x=1', 'L: x:=2; E: skip;', 'L: x:=2; E: skip;'),
    'duplicate_local': ('g=0', 'x=0,x=1', 'L: skip;', 'L: skip;'),
    'reserved_self': ('g=0', 'self=0', 'L: self:=1;', 'L: self:=1;'),
    'reserved_pc': ('g=0', 'pc=0', 'L: pc:=1;', 'L: pc:=1;'),
    'process_name': ('P=0', 'x=0', 'L: P:=1;', 'L: P:=1;'),
    'label_name': ('g=0', 'x=0', 'P: x:=1;', 'P: x:=1;'),
    'invalid_goto': ('g=0', '', 'L: goto Unknown;', 'L: goto Unknown;'),
    'binder_shadow': ('g=0', 'x=0', 'L: with x=1 do g:=x; end with;', 'L: with (x=1) { g:=x; };'),
}
for syntax in ('P', 'C'):
    for name, (glob, local, p, c) in cases.items():
        for domain, binding in (('id', '=1'), ('set', '\\in{1,2}')):
            variants = [('default', [])]
            if name in ('implicit', 'partial_labels'):
                variants.append(('label', ['-label']))
            for option, flags in variants:
                ident = f'Proc_{syntax}_{domain}_{name}_{option}'
                globals_ = f'variables {glob}; ' if glob else ''
                locals_ = f'variables {local}; ' if local else ''
                process = (f'process P {binding} {locals_}begin {p} end process;' if syntax == 'P'
                           else f'process (P {binding}) {{ {locals_} {c} }}')
                algorithm = f'--algorithm A {globals_}{process} end algorithm;' if syntax == 'P' else f'--algorithm A {{ {globals_}{process} }}'
                (base / 'cases' / (ident+'.tla')).write_text(f'---- MODULE {ident} ----\nEXTENDS Naturals, TLC\n(* {algorithm} *)\n====\n')
                rows.append({'id': ident, 'options': flags})
                # Preserve the original C selection's misplaced local declarations
                # as rejection controls, then add the actual C grammar form.
                if syntax == 'C' and local:
                    fixed = ident.replace('Proc_C_', 'Proc_C_fixed_')
                    fixed_process = f'process (P {binding}) {locals_} {{ {c} }}'
                    fixed_algorithm = f'--algorithm A {{ {globals_}{fixed_process} }}'
                    (base/'cases'/(fixed+'.tla')).write_text(f'---- MODULE {fixed} ----\nEXTENDS Naturals, TLC\n(* {fixed_algorithm} *)\n====\n')
                    rows.append({'id':fixed,'options':flags})
    for extra, process in (
        ('empty', '\\in{}'), ('duplicate_ids', '\\in{1,1}'),
        ('string', '="p"'), ('interval', '\\in1..2'),
        ('expression_id', '=1+1'), ('tuple_id', '=<<1,2>>'),
    ):
        ident = f'Proc_{syntax}_{extra}'
        body = f'process P {process} begin L: skip; E: skip; end process; end algorithm;' if syntax=='P' else f'{{ process (P {process}) {{ L: skip; E: skip; }} }}'
        (base/'cases'/(ident+'.tla')).write_text(f'---- MODULE {ident} ----\nEXTENDS Naturals,TLC\n(* --algorithm A {body} *)\n====\n')
        rows.append({'id':ident,'options':[]})
    for fair in ('fair', 'fair +'):
        ident = f'Proc_{syntax}_fair'+ ('_strong' if '+' in fair else '_weak')
        body = f'{fair} process P \\in{{1,2}} begin L: skip; E: skip; end process; end algorithm;' if syntax=='P' else f'{{ {fair} process (P \\in{{1,2}}) {{ L: skip; E: skip; }} }}'
        (base/'cases'/(ident+'.tla')).write_text(f'---- MODULE {ident} ----\nEXTENDS Naturals,TLC\n(* --algorithm A {body} *)\n====\n')
        rows.append({'id':ident,'options':[]})
(base/'selection.json').write_text(json.dumps({'schema_version':1,'rows':rows},indent=2)+'\n')
print(len(rows))
