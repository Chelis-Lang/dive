---- MODULE Macro_C_selector_arg ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=[i \in 1..2 |-> 0], y=0; macro Put(v, a) { v[a]:=a+1; }; { L: Put(x,1); E: skip; } } *)
====
