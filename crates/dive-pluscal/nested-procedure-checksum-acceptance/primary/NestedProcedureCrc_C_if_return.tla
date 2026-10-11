---- MODULE NestedProcedureCrc_C_if_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:if (a=0) { return; } else { x:=a; }; R:return; }; { L:call Put(2); E:skip; } } *)
====
