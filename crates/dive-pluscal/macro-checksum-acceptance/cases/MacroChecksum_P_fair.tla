---- MODULE MacroChecksum_P_fair ----
EXTENDS Integers,TLC,Sequences
(* --fair algorithm A variables x=0, y=0; macro Put(v,a) begin v := a; end macro; begin L: Put(x,1); E:skip; end algorithm; *)
====
