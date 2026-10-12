---- MODULE NestedProcedureCrc_C_either_if_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:either { if (x=0) { call Put(2); goto E; } else { skip; }; } or { skip; }; E:skip; } } *)
====
