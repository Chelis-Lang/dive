---- MODULE MacroExtra_C_latin1 ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := <<"é",a>>; }; { L: Put(x, 1); E: skip; } } *)
====
