---- MODULE MacroExtra_C_with_rename ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { with (t = a) { v:=t; } }; { L: Put(x, t); E: skip; } } *)
====
