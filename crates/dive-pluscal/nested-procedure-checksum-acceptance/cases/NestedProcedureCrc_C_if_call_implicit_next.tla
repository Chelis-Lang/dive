---- MODULE NestedProcedureCrc_C_if_call_implicit_next ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:if (x=0) { call Put(2); } else { call Put(3); }; E:skip; } } *)
====
