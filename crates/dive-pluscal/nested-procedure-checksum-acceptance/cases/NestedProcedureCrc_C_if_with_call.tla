---- MODULE NestedProcedureCrc_C_if_with_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:if (x=0) { with (q=2) { call Put(q); goto E; }; } else { skip; }; E:skip; } } *)
====
