---- MODULE Macro_C_assert ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { assert a>0; v:=a; }; { L: Put(x, 1); E: skip; } } *)
====
