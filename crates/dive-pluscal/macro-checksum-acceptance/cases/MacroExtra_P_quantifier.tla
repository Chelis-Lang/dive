---- MODULE MacroExtra_P_quantifier ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := \A a \in {1,2}: a>0; end macro; begin L: Put(x, 1); E: skip; end algorithm; *)
====
