---- MODULE MacroChecksum_C_source_label ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot M)
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { Put(x,1); skip; } } *)
====
