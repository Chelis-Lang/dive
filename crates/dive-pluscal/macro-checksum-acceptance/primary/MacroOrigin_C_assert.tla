---- MODULE MacroOrigin_C_assert ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { assert a>0; v:=a; }; macro Outer(v,a) { Put(v,a); }; { Outer(x, 1); skip; } } *)
====
