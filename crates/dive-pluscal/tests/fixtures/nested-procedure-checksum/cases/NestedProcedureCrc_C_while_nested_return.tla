---- MODULE NestedProcedureCrc_C_while_nested_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:while (a=0) { if (a=0) { return; } else { skip; }; }; R:return; }; { L:call Put(2); E:skip; } } *)
====
