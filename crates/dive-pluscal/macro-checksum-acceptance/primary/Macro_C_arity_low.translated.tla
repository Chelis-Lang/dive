---- MODULE Macro_C_arity_low ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: Put(x); E: skip; } } *)
====
