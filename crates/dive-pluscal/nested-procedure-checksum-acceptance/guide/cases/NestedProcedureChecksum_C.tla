---- MODULE NestedProcedureChecksum_C ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:with (q=2) { if (x=0) { call Put(q); goto E; } else { skip; }; }; E:skip; } } *)
====
