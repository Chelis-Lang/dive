---- MODULE MacroExtra_C_free_global ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := y+a; }; { L: Put(x, 1); E: skip; } } *)
====
