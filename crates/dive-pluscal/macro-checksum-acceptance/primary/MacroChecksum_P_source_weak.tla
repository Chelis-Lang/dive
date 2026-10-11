---- MODULE MacroChecksum_P_source_weak ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-wf)
(* --algorithm A variables x=0, y=0; macro Put(v,a) begin v := a; end macro; begin L: Put(x,1); E:skip; end algorithm; *)
====
