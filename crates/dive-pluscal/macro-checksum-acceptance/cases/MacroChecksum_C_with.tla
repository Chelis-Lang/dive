---- MODULE MacroChecksum_C_with ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { with (k = a) { v := k; } }; { L: Put(x, 1); E: skip; } } *)
====
