---- MODULE NestedProcedureCrc_C_while_nested_with_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:while (x=0) { with (q=2) { call Put(q); goto B; }; B:skip; }; E:skip; } } *)
====
