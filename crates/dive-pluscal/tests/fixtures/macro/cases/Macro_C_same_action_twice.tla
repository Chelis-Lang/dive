---- MODULE Macro_C_same_action_twice ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: Put(x,1); Put(x,2); E: skip; } } *)
====
