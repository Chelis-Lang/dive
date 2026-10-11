---- MODULE MacroOrigin_C_if ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { if (a>0) { v:=a; } else { v:=0; }; }; macro Outer(v,a) { Put(v,a); }; { Outer(x, 1); skip; } } *)
====
