---- MODULE Macro_C_await ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { await a>0; v:=a; }; { L: Put(x, 1); E: skip; } } *)
====
