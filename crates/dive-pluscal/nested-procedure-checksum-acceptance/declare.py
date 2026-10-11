from pathlib import Path
import hashlib,json,shutil
W=Path('/Users/jeff/code/dive-pluscal-definition-forms-work/crates/dive-pluscal/procedure-checksum-acceptance');O=Path(__file__).resolve().parent;C=O/'cases';C.mkdir(exist_ok=True)
rows=json.loads((W/'selection.json').read_bytes())['rows']
for row in rows:shutil.copy2(W/'cases'/(row['id']+'.tla'),C/(row['id']+'.tla'))
# Paired syntax selection is fixed before any new production edits or capture.
pairs=[
('if_call_one','L:if (x=0) { call Put(2); goto E; } else { skip; }; E:skip;','L:if x=0 then call Put(2); goto E; else skip; end if; E:skip;'),
('if_call_empty_else','L:if (x=0) { call Put(2); goto E; } else { }; E:skip;','L:if x=0 then call Put(2); goto E; else end if; E:skip;'),
('if_call_no_else','L:if (x=0) { call Put(2); goto E; }; E:skip;','L:if x=0 then call Put(2); goto E; end if; E:skip;'),
('if_call_implicit_next','L:if (x=0) { call Put(2); } else { call Put(3); }; E:skip;','L:if x=0 then call Put(2); else call Put(3); end if; E:skip;'),
('with_call_implicit_next','L:with (q=2) { call Put(q); }; E:skip;','L:with q=2 do call Put(q); end with; E:skip;'),
('with_if_call','L:with (q=2) { if (x=0) { call Put(q); goto E; } else { skip; }; }; E:skip;','L:with q=2 do if x=0 then call Put(q); goto E; else skip; end if; end with; E:skip;'),
('either_call_mixed','L:either { call Put(2); goto E; } or { skip; }; E:skip;','L:either call Put(2); goto E; or skip; end either; E:skip;'),
('either_call_implicit_next','L:either { call Put(2); } or { call Put(3); }; E:skip;','L:either call Put(2); or call Put(3); end either; E:skip;'),
('if_with_call','L:if (x=0) { with (q=2) { call Put(q); goto E; }; } else { skip; }; E:skip;','L:if x=0 then with q=2 do call Put(q); goto E; end with; else skip; end if; E:skip;'),
('either_if_call','L:either { if (x=0) { call Put(2); goto E; } else { skip; }; } or { skip; }; E:skip;','L:either if x=0 then call Put(2); goto E; else skip; end if; or skip; end either; E:skip;'),
('while_direct_call','L:while (x=0) { call Put(2); B:skip; }; E:skip;','L:while x=0 do call Put(2); B:skip; end while; E:skip;'),
('while_nested_if_call','L:while (x=0) { if (x=0) { call Put(2); goto B; } else { skip; }; B:skip; }; E:skip;','L:while x=0 do if x=0 then call Put(2); goto B; else skip; end if; B:skip; end while; E:skip;'),
('while_nested_with_call','L:while (x=0) { with (q=2) { call Put(q); goto B; }; B:skip; }; E:skip;','L:while x=0 do with q=2 do call Put(q); goto B; end with; B:skip; end while; E:skip;'),
('while_nested_either_call','L:while (x=0) { either { call Put(2); goto B; } or { skip; }; B:skip; }; E:skip;','L:while x=0 do either call Put(2); goto B; or skip; end either; B:skip; end while; E:skip;'),
('call_after_assignment','L:if (x=0) { x:=1; call Put(2); goto E; } else { skip; }; E:skip;','L:if x=0 then x:=1; call Put(2); goto E; else skip; end if; E:skip;'),
('nested_arg_record','L:with (q=[v |-> 2]) { call Put(q.v); goto E; }; E:skip;','L:with q=[v |-> 2] do call Put(q.v); goto E; end with; E:skip;'),
]
for name,cb,pb in pairs:
 for syn,body in [('C',cb),('P',pb)]:
  ident='NestedProcedureCrc_'+syn+'_'+name
  algorithm=('--algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { '+body+' } }' if syn=='C'else '--algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin '+body+' end algorithm;')
  raw=('---- MODULE '+ident+' ----\nEXTENDS Integers, Sequences, TLC\n(* '+algorithm+' *)\n====\n').encode();(C/(ident+'.tla')).write_bytes(raw)
  rows.append({'id':ident,'flags':[],'input_sha256':hashlib.sha256(raw).hexdigest(),'origin':'nested-call-shared-vector-delta'})
selection={'schema_version':1,'selected':len(rows),'declared_before_production':True,'historical_rows':132,'delta_rows':len(rows)-132,'primary_capture_status':'pending; not a conformance report','scope':'Nested single sequential procedure shared-vector mutation; every prior row retained. Additional return/slot/source-position cases must be declared in an expanded selection before production.' ,'rows':rows}
(O/'selection.json').write_text(json.dumps(selection,indent=2)+'\n');print(len(rows))
return_pairs=[
('if_return','P:if (a=0) { return; } else { x:=a; }; R:return;','P:if a=0 then return; else x:=a; end if; R:return;'),
('with_return','P:with (q=a) { return; };','P:with q=a do return; end with;'),
('either_return','P:either { return; } or { x:=a; }; R:return;','P:either return; or x:=a; end either; R:return;'),
('nested_with_if_return','P:with (q=a) { if (q=0) { return; } else { x:=q; }; }; R:return;','P:with q=a do if q=0 then return; else x:=q; end if; end with; R:return;'),
('while_direct_return','P:while (a=0) { return; }; R:return;','P:while a=0 do return; end while; R:return;'),
('while_nested_return','P:while (a=0) { if (a=0) { return; } else { skip; }; }; R:return;','P:while a=0 do if a=0 then return; else skip; end if; end while; R:return;'),
('if_return_both','P:if (a=0) { return; } else { return; };','P:if a=0 then return; else return; end if;'),
('either_return_both','P:either { return; } or { return; };','P:either return; or return; end either;'),
]
for name,cb,pb in return_pairs:
 for syn,body in [('C',cb),('P',pb)]:
  ident='NestedProcedureCrc_'+syn+'_'+name
  algorithm=('--algorithm A { variable x=0; procedure Put(a=0) { '+body+' }; { L:call Put(2); E:skip; } }' if syn=='C'else '--algorithm A variable x=0; procedure Put(a=0) begin '+body+' end procedure; begin L:call Put(2); E:skip; end algorithm;')
  raw=('---- MODULE '+ident+' ----\nEXTENDS Integers, Sequences, TLC\n(* '+algorithm+' *)\n====\n').encode();(C/(ident+'.tla')).write_bytes(raw)
  rows.append({'id':ident,'flags':[],'input_sha256':hashlib.sha256(raw).hexdigest(),'origin':'nested-return-shared-vector-delta'})
selection.update(selected=len(rows),delta_rows=len(rows)-132,scope='Nested single sequential procedure shared-vector mutation with Call/CallGoto/Return across If/With/Either/While; every prior row retained. Scope credit requires captured stages and separate artifact/model/witness controls.')
(O/'selection.json').write_text(json.dumps(selection,indent=2)+'\n');print(len(rows))
