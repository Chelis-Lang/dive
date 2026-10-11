---- MODULE NestedProcedureCrc_C_if_call_empty_else ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:if (x=0) { call Put(2); goto E; } else { }; E:skip; } } *)
====
