---- MODULE NestedProcedureCrc_C_with_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:with (q=a) { return; }; }; { L:call Put(2); E:skip; } } *)
====
