---- MODULE MacroExtra_C_auto_while ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { while (v<a) { v:=v+1; }; }; { Put(x, 1); skip; } } *)
====
