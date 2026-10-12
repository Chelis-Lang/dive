---- MODULE Macro_C_unknown ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: Nope(x,1); E: skip; } } *)
====
