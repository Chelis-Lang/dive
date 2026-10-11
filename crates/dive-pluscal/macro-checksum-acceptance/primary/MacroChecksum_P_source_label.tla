---- MODULE MacroChecksum_P_source_label ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot M)
(* --algorithm A variables x=0, y=0; macro Put(v,a) begin v := a; end macro; begin Put(x,1); skip; end algorithm; *)
====
