---- MODULE Macro_C_zero ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put() { x:=1; }; { L: Put(); E: skip; } } *)
====
