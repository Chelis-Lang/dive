---- MODULE MacroExtra_C_quantifier ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := \A a \in {1,2}: a>0; }; { L: Put(x, 1); E: skip; } } *)
====
