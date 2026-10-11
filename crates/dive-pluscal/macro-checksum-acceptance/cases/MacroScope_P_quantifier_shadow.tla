---- MODULE MacroScope_P_quantifier_shadow ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0,b=1; macro Put(v,a) begin v:=\A a \in {1,2}: a>0; end macro; begin L: Put(x,2); E: skip; end algorithm; *)
====
