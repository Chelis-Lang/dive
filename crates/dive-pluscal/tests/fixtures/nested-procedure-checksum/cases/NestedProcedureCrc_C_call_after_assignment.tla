---- MODULE NestedProcedureCrc_C_call_after_assignment ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:if (x=0) { x:=1; call Put(2); goto E; } else { skip; }; E:skip; } } *)
====
