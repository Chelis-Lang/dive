---- MODULE MacroOrigin_C_while ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { while (v<a) { v:=v+1; }; }; macro Outer(v,a) { Put(v,a); }; { Outer(x, 1); skip; } } *)
====
