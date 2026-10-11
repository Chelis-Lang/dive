---- MODULE Macro_C_with_capture ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { with (a = 2) { v:=a; } }; { L: Put(x, 1); E: skip; } } *)
====
