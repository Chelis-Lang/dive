---- MODULE Macro_C_while ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { while (v<a) { v:=v+1; }; }; { L: Put(x, 1); E: skip; } } *)
====
