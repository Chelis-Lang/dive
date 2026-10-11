---- MODULE NestedProcedureCrc_C_either_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:either { return; } or { x:=a; }; R:return; }; { L:call Put(2); E:skip; } } *)
====
