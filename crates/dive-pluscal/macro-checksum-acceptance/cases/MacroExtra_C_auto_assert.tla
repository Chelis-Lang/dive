---- MODULE MacroExtra_C_auto_assert ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { assert a>0; v:=a; }; { Put(x, 1); skip; } } *)
====
