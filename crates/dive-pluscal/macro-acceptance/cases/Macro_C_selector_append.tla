---- MODULE Macro_C_selector_append ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=[i \in 1..2 |-> [j \in 1..2 |-> 0]], y=0; macro Put(v, a) { v[2]:=a; }; { L: Put(x[1],2); E: skip; } } *)
====
