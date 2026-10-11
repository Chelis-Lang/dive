---- MODULE Macro_C_duplicate_macro ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; };macro Put(v, a) { v:=a+1; }; { L: Put(x, 1); E: skip; } } *)
====
