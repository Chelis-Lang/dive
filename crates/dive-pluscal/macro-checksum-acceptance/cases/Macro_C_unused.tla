---- MODULE Macro_C_unused ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: x:=1; E: skip; } } *)
====
