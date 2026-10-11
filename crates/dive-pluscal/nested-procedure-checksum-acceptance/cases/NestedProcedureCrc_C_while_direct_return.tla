---- MODULE NestedProcedureCrc_C_while_direct_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:while (a=0) { return; }; R:return; }; { L:call Put(2); E:skip; } } *)
====
