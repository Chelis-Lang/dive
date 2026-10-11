---- MODULE NestedProcedureCrc_C_either_call_implicit_next ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:either { call Put(2); } or { call Put(3); }; E:skip; } } *)
====
