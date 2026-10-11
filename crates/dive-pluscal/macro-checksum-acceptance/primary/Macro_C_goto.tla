---- MODULE Macro_C_goto ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { goto E; }; { L: Put(x, 1); E: skip; } } *)
====
