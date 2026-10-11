---- MODULE MacroChecksum_C_tabs ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A { variables x=0, y=0; macro Put(v, a) {	 v := a; }; { L: Put(x, 1); E: skip; } } *)
====
