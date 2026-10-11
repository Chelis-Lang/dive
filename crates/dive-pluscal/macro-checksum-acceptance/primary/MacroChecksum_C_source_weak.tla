---- MODULE MacroChecksum_C_source_weak ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf)
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: Put(x, 1); E: skip; } } *)
====
