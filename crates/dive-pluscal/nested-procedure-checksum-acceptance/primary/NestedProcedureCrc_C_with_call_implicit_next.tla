---- MODULE NestedProcedureCrc_C_with_call_implicit_next ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:with (q=2) { call Put(q); }; E:skip; } } *)
====
