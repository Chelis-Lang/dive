---- MODULE Macro_C_nested_rhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := 2*a + a; }; { L: Put(x, 1+2); E: skip; } } *)
====
