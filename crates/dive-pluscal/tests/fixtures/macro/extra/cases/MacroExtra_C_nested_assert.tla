---- MODULE MacroExtra_C_nested_assert ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { assert a>0; v := a; };macro Outer(v, a) { Put(v,a+1); }; { L: Outer(x,2); E: skip; } } *)
====
