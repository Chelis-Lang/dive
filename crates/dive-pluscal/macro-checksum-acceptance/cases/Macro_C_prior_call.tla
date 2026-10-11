---- MODULE Macro_C_prior_call ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; };macro Outer(v, a) { Put(v,a+1); }; { L: Outer(x,2); E: skip; } } *)
====
