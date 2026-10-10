---- MODULE MacroExtra_C_nested_unused ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; };macro Outer(v,a) { Put(v,a); }; { L: x:=1; E: skip; } } *)
====
