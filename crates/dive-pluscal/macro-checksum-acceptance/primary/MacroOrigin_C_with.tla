---- MODULE MacroOrigin_C_with ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { with (t = a) { v:=t; } }; { Put(x, 1); skip; } } *)
====
