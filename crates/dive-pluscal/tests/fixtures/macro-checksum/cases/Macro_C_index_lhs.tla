---- MODULE Macro_C_index_lhs ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=[i \in 1..2 |-> 0], y=0; macro Put(v, a) { v := a; }; { L: Put(x[1],2); E: skip; } } *)
====
