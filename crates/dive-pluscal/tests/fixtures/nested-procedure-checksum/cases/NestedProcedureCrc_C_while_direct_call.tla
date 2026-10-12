---- MODULE NestedProcedureCrc_C_while_direct_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:while (x=0) { call Put(2); B:skip; }; E:skip; } } *)
====
