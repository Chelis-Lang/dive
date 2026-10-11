---- MODULE Macro_C_if ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { if (a>0) { v:=a; } else { v:=0; }; }; { L: Put(x, 1); E: skip; } } *)
====
