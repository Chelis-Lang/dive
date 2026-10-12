---- MODULE Macro_C_auto_label ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { Put(x,1); skip; } } *)
====
