---- MODULE Macro_C_forward_call ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Outer(v, a) { Put(v,a); };macro Put(v, a) { v := a; }; { L: Outer(x,1); E: skip; } } *)
====
