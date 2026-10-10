---- MODULE Macro_C_field_rhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(a) { x := [a |-> a].a; }; { L: Put(2); E: skip; } } *)
====
