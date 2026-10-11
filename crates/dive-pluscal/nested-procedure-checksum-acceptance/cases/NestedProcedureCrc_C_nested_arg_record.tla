---- MODULE NestedProcedureCrc_C_nested_arg_record ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:with (q=[v |-> 2]) { call Put(q.v); goto E; }; E:skip; } } *)
====
