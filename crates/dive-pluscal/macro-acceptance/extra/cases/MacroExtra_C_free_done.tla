---- MODULE MacroExtra_C_free_done ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; };macro Unused() { goto Done; }; { L: Put(x, 1); E: skip; } } *)
====
