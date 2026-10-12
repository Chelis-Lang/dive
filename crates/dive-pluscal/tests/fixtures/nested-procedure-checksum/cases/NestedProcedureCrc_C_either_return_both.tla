---- MODULE NestedProcedureCrc_C_either_return_both ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:either { return; } or { return; }; }; { L:call Put(2); E:skip; } } *)
====
