---- MODULE NestedProcedureCrc_C_nested_with_if_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:with (q=a) { if (q=0) { return; } else { x:=q; }; }; R:return; }; { L:call Put(2); E:skip; } } *)
====
