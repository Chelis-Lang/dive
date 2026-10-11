---- MODULE NestedProcedureCrc_C_while_nested_if_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:while (x=0) { if (x=0) { call Put(2); goto B; } else { skip; }; B:skip; }; E:skip; } } *)
====
