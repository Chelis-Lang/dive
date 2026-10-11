---- MODULE Macro_C_literal_lhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: Put(1,2); E: skip; } } *)
====
