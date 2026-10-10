---- MODULE Macro_C_field_lhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=[a |-> 0], y=0; macro Put(v, a) { v := a; }; { L: Put(x.a,2); E: skip; } } *)
====
