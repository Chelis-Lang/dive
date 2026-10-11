---- MODULE Macro_C_either ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { either { v:=a; } or { v:=0; }; }; { L: Put(x, 1); E: skip; } } *)
====
