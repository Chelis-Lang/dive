---- MODULE MacroChecksum_C_crlf ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: Put(x, 1); E: skip; } } *)
====
