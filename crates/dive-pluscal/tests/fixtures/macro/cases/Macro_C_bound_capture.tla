---- MODULE Macro_C_bound_capture ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(a) { x := {a \in 1..2 : a>0}; }; { L: Put(2); E: skip; } } *)
====
