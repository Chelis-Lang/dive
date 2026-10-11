---- MODULE MacroChecksum_C_nested ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; macro Twice(v,a) { Put(v,a+1); }; { L: Twice(x,1); E:skip; } } *)
====
