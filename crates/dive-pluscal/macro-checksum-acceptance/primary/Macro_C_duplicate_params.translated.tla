---- MODULE Macro_C_duplicate_params ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(a, a) { x := a; }; { L: Put(2,3); E: skip; } } *)
====
